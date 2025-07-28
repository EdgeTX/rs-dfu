use std::{num::NonZeroU8, time::Duration};

use nusb::{self, MaybeFuture};

use crate::memory::*;

pub struct DfuInterface {
    config: u8,
    interface: u8,
    alt_setting: u8,
    layout: DfuMemory,
}

impl DfuInterface {
    pub(crate) fn new(
        device: &nusb::Device,
        config: u8,
        interface: u8,
        alt_setting: u8,
        name_idx: NonZeroU8,
    ) -> Option<Self> {
        let intf_str =
            get_string_descriptor(device, name_idx, crate::DEFAULT_TIMEOUT)?;

        let layout = parse_memory_layout(&intf_str)?;
        Some(Self {
            config,
            interface,
            alt_setting,
            layout,
        })
    }

    pub fn config(&self) -> u8 {
        self.config
    }
    pub fn interface(&self) -> u8 {
        self.interface
    }
    pub fn alt_setting(&self) -> u8 {
        self.alt_setting
    }
    pub fn layout(&self) -> &DfuMemory {
        &self.layout
    }

    pub fn find_segments(
        &self,
        start_address: u32,
        end_address: u32,
    ) -> Vec<DfuMemSegment> {
        self.layout.find_segments(start_address, end_address)
    }
}

fn get_string_descriptor(
    device: &nusb::Device,
    desc_index: NonZeroU8,
    timeout: Duration,
) -> Option<String> {
    let language: u16 = device
        .get_string_descriptor_supported_languages(timeout)
        .wait()
        .ok()?
        .next()
        .unwrap_or(nusb::descriptors::language_id::US_ENGLISH);

    device
        .get_string_descriptor(desc_index, language, timeout)
        .wait()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_segments() {
        let layout =
            parse_memory_layout("@Internal Flash   /0x08000000/8*08Kg")
                .unwrap();

        assert!(!layout.find_segments(0x08000000, 0x08001000).is_empty());
        assert!(!layout.find_segments(0x08000000, 0x0800e9a0).is_empty());
        assert!(!layout.find_segments(0x08001000, 0x0800e9a0).is_empty());

        let layout = parse_memory_layout(
            "@Internal Flash  /0x08000000/04*016Kg,01*064Kg,07*128Kg",
        )
        .unwrap();

        assert_eq!(layout.find_segments(0x08000000, 0x08020000).len(), 3);
        assert_eq!(layout.find_segments(0x08000000, 0x0800e9a0).len(), 1);
        assert_eq!(layout.find_segments(0x08001000, 0x0800e9a0).len(), 1);
    }
}
