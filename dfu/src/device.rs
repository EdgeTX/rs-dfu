use std::time::{Duration, Instant};

use nusb::{self, MaybeFuture};

use crate::{DfuConnection, DfuError, descriptor::*, interface::*};

const DFU_CLASS: u8 = 0xFE;
const DFU_SUBCLASS: u8 = 0x1;
const DFU_PROTOCOL_RUNTIME: u8 = 0x1;

const RECONNECT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DISCONNECT_GRACE: Duration = Duration::from_secs(2);

/// Whether an interface can be used to talk to a device in DFU mode.
///
/// Runtime interfaces are excluded: they are exposed by devices in normal
/// operation, which may not even be accessible to the user, and they would
/// first have to be detached into DFU mode, which is not supported.
fn is_dfu_mode_interface(class: u8, subclass: u8, protocol: u8) -> bool {
    class == DFU_CLASS
        && subclass == DFU_SUBCLASS
        && protocol != DFU_PROTOCOL_RUNTIME
}

/// DFU device representation
pub struct DfuDevice {
    dev: nusb::DeviceInfo,
    interfaces: Vec<DfuInterface>,
}

impl DfuDevice {
    fn from_device_info(
        device: nusb::DeviceInfo,
    ) -> Result<Option<Self>, DfuError> {
        let open_dev: nusb::Device =
            device.open().wait().map_err(|source| DfuError::Open {
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                source,
            })?;
        let dfu_interfaces: Vec<DfuInterface> = open_dev
            .configurations()
            .flat_map(|configuration| {
                let open_dev = open_dev.clone();
                let config = configuration.configuration_value();
                configuration.interface_alt_settings().filter_map(
                    move |alt_setting| {
                        if !is_dfu_mode_interface(
                            alt_setting.class(),
                            alt_setting.subclass(),
                            alt_setting.protocol(),
                        ) {
                            None
                        } else {
                            DfuInterface::new(
                                &open_dev,
                                config,
                                alt_setting.interface_number(),
                                alt_setting.alternate_setting(),
                                alt_setting.string_index()?,
                            )
                        }
                    },
                )
            })
            .collect();

        if dfu_interfaces.is_empty() {
            Ok(None)
        } else {
            Ok(Some(DfuDevice {
                dev: device,
                interfaces: dfu_interfaces,
            }))
        }
    }

    pub fn device_info(&self) -> &nusb::DeviceInfo {
        &self.dev
    }

    pub fn id(&self) -> nusb::DeviceId {
        self.dev.id()
    }

    pub fn bus_id(&self) -> &str {
        self.dev.bus_id()
    }

    pub fn device_address(&self) -> u8 {
        self.dev.device_address()
    }

    pub fn vendor_id(&self) -> u16 {
        self.dev.vendor_id()
    }

    pub fn product_id(&self) -> u16 {
        self.dev.product_id()
    }

    pub fn product_string(&self) -> Option<&str> {
        self.dev.product_string()
    }

    pub fn device_version(&self) -> String {
        bcd_version_string(self.dev.device_version())
    }

    /// DFU interfaces and alternate settings combined
    pub fn interfaces(&self) -> &Vec<DfuInterface> {
        &self.interfaces
    }

    pub(crate) fn open(&self) -> Result<nusb::Device, DfuError> {
        Ok(self.dev.open().wait()?)
    }

    pub fn is_dfuse(&self) -> bool {
        self.dfu_descriptor().ok().unwrap_or_default().dfu_version()
            == DFUSE_VERSION_NUMBER
    }

    /// Query the DFU descriptor for this device. If no descriptor can be found,
    /// [DfuDescriptor::default()] is returned.
    pub fn dfu_descriptor(&self) -> Result<DfuDescriptor, DfuError> {
        let open_dev = self.open()?;
        Ok(
            match open_dev.configurations().find_map(|config| {
                config.interface_alt_settings().find_map(|alt_setting| {
                    alt_setting.descriptors().find(is_dfu_descriptor)
                })
            }) {
                Some(dfu_desc) => DfuDescriptor::new(&dfu_desc),
                None => DfuDescriptor::default(),
            },
        )
    }

    /// Find a matching interface and alternate setting
    pub fn find_interface(
        &self,
        start_address: u32,
        end_address: Option<u32>,
    ) -> Result<&DfuInterface, DfuError> {
        self.interfaces
            .iter()
            .find(|intf| {
                let segments = intf.find_segments(start_address, end_address);
                !segments.is_empty()
                // verify boundaries
                && start_address >= segments.first().unwrap().start_addr()
                && end_address.is_none_or(|addr| addr <= segments.last().unwrap().end_addr())
            })
            .ok_or(DfuError::NoMemorySegments)
    }

    /// Return the start address of the first alternate setting
    pub fn get_default_start_address(&self) -> u32 {
        self.interfaces[0].layout().segments[0].start_addr()
    }

    /// Connect to the DFU interface
    ///
    /// Allows for interacting with the DFU interface (ex: read / write firmware).
    pub fn connect(
        &self,
        interface: u8,
        alt_setting: u8,
    ) -> Result<DfuConnection, DfuError> {
        let xfer_size = self.dfu_descriptor()?.transfer_size();
        let dev = self.open()?;
        let interface = dev.claim_interface(interface).wait()?;
        interface.set_alt_setting(alt_setting).wait()?;
        Ok(DfuConnection::new(interface, xfer_size))
    }

    /// Wait for this device to come back after a reboot
    ///
    /// Returns the last error met while opening the device if it is not usable
    /// before `timeout` elapses, or [DfuError::Timeout] if it never came back.
    pub fn wait_for_reconnect(
        &self,
        timeout: Duration,
    ) -> Result<DfuDevice, DfuError> {
        let mut reconnect = DfuReconnect::new(self);
        while reconnect.started.elapsed() < timeout {
            if let Some(device) = reconnect.poll()? {
                return Ok(device);
            }
        }
        Err(reconnect.last_error.unwrap_or(DfuError::Timeout))
    }
}

/// Physical port a device is attached to, which remains the same when the
/// device re-enumerates, unlike its bus address.
struct DevicePort {
    vendor_id: u16,
    product_id: u16,
    bus_id: String,
    port_chain: Vec<u8>,
}

impl DevicePort {
    fn matches(
        &self,
        vendor_id: u16,
        product_id: u16,
        bus_id: &str,
        port_chain: &[u8],
    ) -> bool {
        self.vendor_id == vendor_id
            && self.product_id == product_id
            && self.bus_id == bus_id
            && self.port_chain == port_chain
    }
}

/// Whether a device found on the expected port can be opened. The device we
/// rebooted may still be listed for a moment, so it is only accepted if it
/// has been seen leaving, or has not left after [DISCONNECT_GRACE] (the gap
/// can be missed between two polls if its ID does not change on this platform).
fn can_open(
    is_previous_id: bool,
    disconnected: bool,
    elapsed: Duration,
) -> bool {
    !is_previous_id || disconnected || elapsed >= DISCONNECT_GRACE
}

/// Tracks a device while it reboots, for callers polling on their own terms.
/// Prefer [DfuDevice::wait_for_reconnect] otherwise.
pub struct DfuReconnect {
    port: DevicePort,
    previous_id: nusb::DeviceId,
    started: Instant,
    disconnected: bool,
    last_error: Option<DfuError>,
}

impl DfuReconnect {
    /// Start tracking `device`, which should just have been rebooted.
    pub fn new(device: &DfuDevice) -> Self {
        DfuReconnect {
            port: DevicePort {
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                bus_id: device.bus_id().to_string(),
                port_chain: device.dev.port_chain().to_vec(),
            },
            previous_id: device.id(),
            started: Instant::now(),
            disconnected: false,
            last_error: None,
        }
    }

    /// Last error met while opening the device, if any.
    pub fn last_error(&self) -> Option<&DfuError> {
        self.last_error.as_ref()
    }

    /// Check once for the device, sleeping briefly if it is not available yet.
    ///
    /// Failing to open it is not an error here, as permissions may not be
    /// applied yet right after it re-enumerates.
    pub fn poll(&mut self) -> Result<Option<DfuDevice>, DfuError> {
        let candidate = nusb::list_devices().wait()?.find(|dev| {
            self.port.matches(
                dev.vendor_id(),
                dev.product_id(),
                dev.bus_id(),
                dev.port_chain(),
            ) && is_dfu_device(dev)
        });
        match candidate {
            None => self.disconnected = true,
            Some(dev)
                if can_open(
                    dev.id() == self.previous_id,
                    self.disconnected,
                    self.started.elapsed(),
                ) =>
            {
                match DfuDevice::from_device_info(dev) {
                    Ok(Some(device)) => return Ok(Some(device)),
                    Ok(None) => {}
                    Err(err) => {
                        log::debug!("Rebooted device not ready: {err}");
                        self.last_error = Some(err);
                    }
                }
            }
            Some(_) => {}
        }
        std::thread::sleep(RECONNECT_POLL_INTERVAL);
        Ok(None)
    }
}

fn is_dfu_descriptor(desc: &nusb::descriptors::Descriptor) -> bool {
    desc.descriptor_len() == DFU_DESC_LEN
        && desc.descriptor_type() == DFU_DESC_TYPE
}

fn is_dfu_device(dev: &nusb::DeviceInfo) -> bool {
    dev.interfaces()
        .any(|i| is_dfu_mode_interface(i.class(), i.subclass(), i.protocol()))
}

pub fn find_dfu_devices(
    vid: Option<u16>,
    pid: Option<u16>,
) -> Result<Vec<DfuDevice>, DfuError> {
    let devices: Vec<nusb::DeviceInfo> = nusb::list_devices()
        .wait()?
        .filter(|dev| {
            vid.is_none_or(|id| dev.vendor_id() == id)
                && pid.is_none_or(|id| dev.product_id() == id)
        })
        .filter(is_dfu_device)
        .collect();
    let mut dfu_devices = Vec::with_capacity(devices.len());
    for device in devices {
        if let Some(dfu_device) = DfuDevice::from_device_info(device)? {
            dfu_devices.push(dfu_device);
        }
    }
    Ok(dfu_devices)
}

fn bcd_version_string(bcd_version: u16) -> String {
    let major = ((bcd_version >> 12) & 0xF) * 10 + ((bcd_version >> 8) & 0xF);
    let minor = ((bcd_version >> 4) & 0xF) * 10 + (bcd_version & 0xF);
    format!("{}.{:02}", major, minor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_dfu_mode_interface() {
        // DFU mode, and DFU 1.0 devices reporting protocol 0
        assert!(is_dfu_mode_interface(DFU_CLASS, DFU_SUBCLASS, 0x2));
        assert!(is_dfu_mode_interface(DFU_CLASS, DFU_SUBCLASS, 0x0));

        assert!(!is_dfu_mode_interface(
            DFU_CLASS,
            DFU_SUBCLASS,
            DFU_PROTOCOL_RUNTIME
        ));
        assert!(!is_dfu_mode_interface(DFU_CLASS, 0x2, 0x2));
        assert!(!is_dfu_mode_interface(0x8, DFU_SUBCLASS, 0x2));
    }

    #[test]
    fn test_device_port_matches() {
        let port = DevicePort {
            vendor_id: 0x0483,
            product_id: 0xdf11,
            bus_id: "3".to_string(),
            port_chain: vec![3, 2],
        };

        assert!(port.matches(0x0483, 0xdf11, "3", &[3, 2]));

        // same model on another port, or another hub level
        assert!(!port.matches(0x0483, 0xdf11, "3", &[3, 1]));
        assert!(!port.matches(0x0483, 0xdf11, "3", &[3, 2, 1]));
        assert!(!port.matches(0x0483, 0xdf11, "4", &[3, 2]));

        // another model on the same port
        assert!(!port.matches(0x0483, 0x5740, "3", &[3, 2]));
        assert!(!port.matches(0x1209, 0xdf11, "3", &[3, 2]));
    }

    #[test]
    fn test_can_open() {
        let early = Duration::ZERO;

        // new enumeration
        assert!(can_open(false, false, early));
        assert!(can_open(false, true, early));

        // same ID: only once it has left, or has not left in time
        assert!(!can_open(true, false, early));
        assert!(can_open(true, true, early));
        assert!(can_open(true, false, DISCONNECT_GRACE));
    }

    #[test]
    fn test_bcd_string() {
        assert_eq!(bcd_version_string(0x0200), "2.00");
        assert_eq!(bcd_version_string(0x0211), "2.11");
        assert_eq!(bcd_version_string(0x2200), "22.00");
    }
}
