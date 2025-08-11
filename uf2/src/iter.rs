use crate::*;

pub struct UF2RangeIterator<'a> {
    block_iter: Option<std::slice::Chunks<'a, u8>>,
    start_address: u32,
    end_address: u32,
    payload: Vec<u8>,
    reboot_address: Option<u32>,
}

#[derive(Default)]
pub struct UF2AddressRange {
    pub start_address: u32,
    pub payload: Vec<u8>,
    pub reboot_address: Option<u32>,
}

impl<'a> UF2RangeIterator<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, UF2DecodeError> {
        if !data.chunks(UF2_BLOCK_SIZE).all(is_uf2_block) {
            Err(UF2DecodeError)
        } else {
            let mut block_iter = data.chunks(UF2_BLOCK_SIZE);
            let block = UF2BlockData::decode(block_iter.next().unwrap())?;
            Ok(UF2RangeIterator {
                block_iter: Some(block_iter),
                start_address: block.flash_address,
                end_address: block.flash_address + (block.payload.len() as u32),
                payload: block.payload.clone(),
                reboot_address: block.get_reboot_address(),
            })
        }
    }

    fn make_range(&mut self) -> UF2AddressRange {
        UF2AddressRange {
            start_address: self.start_address,
            payload: self.payload.clone(),
            reboot_address: self.reboot_address.take(),
        }
    }

    fn reset(&mut self, block: &UF2BlockData) {
        self.start_address = block.flash_address;
        self.end_address = block.flash_address + (block.payload.len() as u32);
        self.payload = block.payload.clone();
        self.reboot_address = block.get_reboot_address();
    }
}

impl<'a> Iterator for UF2RangeIterator<'a> {
    type Item = UF2AddressRange;

    fn next(&mut self) -> Option<Self::Item> {
        for block in self.block_iter.as_mut()?.by_ref() {
            let block = UF2BlockData::decode(block).ok()?;
            if self.end_address != block.flash_address {
                let item = self.make_range();
                self.reset(&block);
                return Some(item);
            } else {
                self.end_address += block.payload.len() as u32;
                self.payload.extend(&block.payload);
            }
        }
        self.block_iter.take();
        if !self.payload.is_empty() {
            Some(self.make_range())
        } else {
            None
        }
    }
}
