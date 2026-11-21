use nusb::{self, MaybeFuture};

use crate::{DfuConnection, DfuError, descriptor::*, interface::*};

const DFU_CLASS: u8 = 0xFE;
const DFU_SUBCLASS: u8 = 0x1;
const DFU_PROTOCOL_RUNTIME: u8 = 0x1;

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
    fn test_bcd_string() {
        assert_eq!(bcd_version_string(0x0200), "2.00");
        assert_eq!(bcd_version_string(0x0211), "2.11");
        assert_eq!(bcd_version_string(0x2200), "22.00");
    }
}
