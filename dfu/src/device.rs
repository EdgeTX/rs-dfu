#![allow(dead_code)]

use nonempty::NonEmpty;
use nusb::{self, MaybeFuture};

use crate::{
    DfuConnection, DfuError, DfuMemSegment, descriptor::*, interface::*,
};

const DFU_CLASS: u8 = 0xFE;
const DFU_SUBCLASS: u8 = 0x1;

/// DFU device representation
pub struct DfuDevice {
    dev: nusb::DeviceInfo,
    interfaces: Vec<DfuInterface>,
}

/// DFU interface and memory segments matching an address range
///
/// It can be obtained with [DfuDevice::find_interface_segments].
///
pub struct DfuInterfaceSegments {
    interface: u8,
    alt_setting: u8,
    segments: NonEmpty<DfuMemSegment>,
}

impl DfuInterfaceSegments {
    pub fn interface(&self) -> u8 {
        self.interface
    }
    pub fn alt_setting(&self) -> u8 {
        self.alt_setting
    }
    pub fn segments(&self) -> &NonEmpty<DfuMemSegment> {
        &self.segments
    }
}

impl DfuDevice {
    fn from_device_info(
        device: nusb::DeviceInfo,
    ) -> Result<Option<Self>, DfuError> {
        let open_dev: nusb::Device = device.open().wait()?;
        let dfu_interfaces: Vec<DfuInterface> = open_dev
            .configurations()
            .flat_map(|configuration| {
                let open_dev = open_dev.clone();
                let config = configuration.configuration_value();
                configuration.interface_alt_settings().filter_map(
                    move |alt_setting| {
                        if alt_setting.class() != DFU_CLASS
                            || alt_setting.subclass() != DFU_SUBCLASS
                        {
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

    /// Find a matching interface, alternate setting and memory segments
    ///
    /// This is required to connect to the correct interface and to retrieve
    /// the list of memory segments to be erased beforehand.
    pub fn find_interface_segments(
        &self,
        start_address: u32,
        end_address: u32,
    ) -> Result<DfuInterfaceSegments, DfuError> {
        self.interfaces
            .iter()
            .find_map(|intf| {
                let segments = intf.find_segments(start_address, end_address);
                if segments.is_empty()
                // verify boundaries
                || start_address < segments.first().unwrap().start_addr()
                || end_address > segments.last().unwrap().end_addr()
                {
                    None
                } else {
                    Some(DfuInterfaceSegments {
                        interface: intf.interface(),
                        alt_setting: intf.alt_setting(),
                        segments: NonEmpty::from_vec(segments).unwrap(),
                    })
                }
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

    fn interface_segments(
        &self,
        interface: u8,
    ) -> Option<NonEmpty<&DfuMemSegment>> {
        Some(
            self.interfaces
                .iter()
                .find(|intf| intf.interface() == interface)?
                .layout()
                .segments
                .as_ref(),
        )
    }
}

fn is_dfu_descriptor(desc: &nusb::descriptors::Descriptor) -> bool {
    desc.descriptor_len() == DFU_DESC_LEN
        && desc.descriptor_type() == DFU_DESC_TYPE
}

fn is_dfu_device(dev: &nusb::DeviceInfo) -> bool {
    dev.interfaces()
        .any(|i| i.class() == DFU_CLASS && i.subclass() == DFU_SUBCLASS)
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
