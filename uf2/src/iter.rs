use crate::*;

pub struct UF2RangeIterator<'a> {
    block_iter: std::slice::Chunks<'a, u8>,
    pending: Option<UF2AddressRange>,
}

#[derive(Default)]
pub struct UF2AddressRange {
    pub start_address: u32,
    pub payload: Vec<u8>,
    pub reboot_address: Option<u32>,
}

impl<'a> UF2RangeIterator<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, UF2DecodeError> {
        if data.is_empty() {
            return Err(UF2DecodeError::new("empty UF2 payload".to_string()));
        }
        // Validate every block before callers can start flashing a range.
        for (i, chunk) in data.chunks(UF2_BLOCK_SIZE).enumerate() {
            let block = UF2BlockData::decode(chunk).map_err(|err| {
                UF2DecodeError::new(format!(
                    "invalid UF2 block at {:#x}: {}",
                    i * UF2_BLOCK_SIZE,
                    err.err
                ))
            })?;
            if block.get_extension(REBOOT_EXTENSION_TAG).is_some()
                && block.get_reboot_address().is_none()
            {
                return Err(UF2DecodeError::new(format!(
                    "invalid reboot extension at {:#x}",
                    i * UF2_BLOCK_SIZE
                )));
            }
            if block.flags.is_main_flash()
                && block.get_reboot_address().is_some()
            {
                return Err(UF2DecodeError::new(format!(
                    "reboot extension on main-flash block at {:#x}",
                    i * UF2_BLOCK_SIZE
                )));
            }
        }

        Ok(UF2RangeIterator {
            block_iter: data.chunks(UF2_BLOCK_SIZE),
            pending: None,
        })
    }
}

impl<'a> Iterator for UF2RangeIterator<'a> {
    type Item = UF2AddressRange;

    fn next(&mut self) -> Option<Self::Item> {
        let mut range = self.pending.take();
        for data in self.block_iter.by_ref() {
            // All blocks were decoded successfully in new().
            let block = UF2BlockData::decode(data)
                .expect("block was validated in UF2RangeIterator::new");
            let reboot_address = if block.is_reboot_block() {
                block.get_reboot_address()
            } else if block.flags.is_main_flash()
                && !block.flags.file_container()
            {
                None
            } else {
                continue;
            };
            if block.payload.is_empty() && reboot_address.is_none() {
                continue;
            }
            let next = UF2AddressRange {
                start_address: block.flash_address,
                payload: block.payload,
                reboot_address,
            };
            if let Some(current) = range.as_mut() {
                if current.reboot_address.is_none()
                    && next.reboot_address.is_none()
                    && current
                        .start_address
                        .checked_add(current.payload.len() as u32)
                        == Some(next.start_address)
                {
                    current.payload.extend(next.payload);
                } else {
                    self.pending = Some(next);
                    return range;
                }
            } else {
                range = Some(next);
            }
        }
        range
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(
        addr: u32,
        payload: &[u8],
        flags: u32,
        reboot: Option<u32>,
    ) -> Vec<u8> {
        let mut data = vec![0; UF2_BLOCK_SIZE];
        for &(offset, magic) in UF2_MAGIC_VALUES {
            data[offset..offset + 4].copy_from_slice(&magic.to_le_bytes());
        }
        let flags = flags
            | if reboot.is_some() {
                UF2Flags::EXTENSION_TAGS_PRESENT
            } else {
                0
            };
        data[8..12].copy_from_slice(&flags.to_le_bytes());
        data[12..16].copy_from_slice(&addr.to_le_bytes());
        data[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        data[UF2_HEADER_SIZE..UF2_HEADER_SIZE + payload.len()]
            .copy_from_slice(payload);
        if let Some(addr) = reboot {
            let offset = UF2_HEADER_SIZE + payload.len();
            data[offset..offset + 4].copy_from_slice(
                &((REBOOT_EXTENSION_TAG << 8) | 8).to_le_bytes(),
            );
            data[offset + 4..offset + 8].copy_from_slice(&addr.to_le_bytes());
        }
        data
    }

    fn ranges(data: &[u8]) -> Vec<UF2AddressRange> {
        UF2RangeIterator::new(data)
            .unwrap_or_else(|err| panic!("{err}"))
            .collect()
    }

    #[test]
    fn merges_contiguous_flash_blocks_and_splits_address_gaps() {
        let data = [
            block(0x08000000, &[1; 4], 0, None),
            block(0x08000004, &[2; 4], 0, None),
            block(0x08000100, &[3; 4], 0, None),
        ]
        .concat();
        let result = ranges(&data);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].start_address, 0x08000000);
        assert_eq!(result[0].payload, [1, 1, 1, 1, 2, 2, 2, 2]);
        assert_eq!(result[1].start_address, 0x08000100);
        assert_eq!(result[1].payload, [3; 4]);
    }

    #[test]
    fn isolates_reboot_blocks_from_contiguous_flash_on_both_sides() {
        let data = [
            block(0x20000000, &[1; 4], 0, None),
            block(
                0x20000004,
                &[2; 4],
                UF2Flags::NOT_MAIN_FLASH,
                Some(0x20000005),
            ),
            block(0x20000008, &[3; 4], 0, None),
        ]
        .concat();
        let result = ranges(&data);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].payload, [1; 4]);
        assert_eq!(result[0].reboot_address, None);
        assert_eq!(result[1].start_address, 0x20000004);
        assert_eq!(result[1].payload, [2; 4]);
        assert_eq!(result[1].reboot_address, Some(0x20000005));
        assert_eq!(result[2].payload, [3; 4]);
        assert_eq!(result[2].reboot_address, None);
    }

    #[test]
    fn skips_non_flash_blocks_at_start_middle_and_end() {
        let metadata =
            block(0x08000004, &[9; 4], UF2Flags::NOT_MAIN_FLASH, None);
        let data = [
            metadata.clone(),
            block(0x08000000, &[1; 4], 0, None),
            metadata.clone(),
            block(0x08000004, &[2; 4], 0, None),
            metadata,
        ]
        .concat();
        let result = ranges(&data);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_address, 0x08000000);
        assert_eq!(result[0].payload, [1, 1, 1, 1, 2, 2, 2, 2]);
    }

    #[test]
    fn metadata_only_input_has_no_flash_ranges() {
        assert!(
            ranges(&block(0, &[9; 4], UF2Flags::NOT_MAIN_FLASH, None))
                .is_empty()
        );
    }

    #[test]
    fn empty_flash_payload_has_no_ranges() {
        assert!(ranges(&block(0x08000000, &[], 0, None)).is_empty());
    }

    #[test]
    fn rejects_reboot_tag_on_main_flash() {
        assert!(
            UF2RangeIterator::new(&block(
                0x08000000,
                &[1; 4],
                0,
                Some(0x20000001)
            ))
            .is_err()
        );
    }

    #[test]
    fn rejects_decode_errors_before_yielding_any_range() {
        let mut corrupt = block(0x08000200, &[3; 4], 0, None);
        corrupt[16..20].copy_from_slice(
            &((UF2_MAX_PAYLOAD_SIZE + 1) as u32).to_le_bytes(),
        );
        let data = [
            block(0x08000000, &[1; 4], 0, None),
            block(0x08000100, &[2; 4], 0, None),
            corrupt,
        ]
        .concat();
        assert!(UF2RangeIterator::new(&data).is_err());
    }

    #[test]
    fn rejects_payload_sizes_that_overlap_the_final_magic() {
        assert!(
            UF2RangeIterator::new(&block(
                0x08000000,
                &[0; UF2_MAX_PAYLOAD_SIZE],
                0,
                None
            ))
            .is_ok()
        );
        for payload_size in 477..=480 {
            let mut data = block(0x08000000, &[], 0, None);
            data[16..20].copy_from_slice(&(payload_size as u32).to_le_bytes());
            assert!(UF2RangeIterator::new(&data).is_err());
        }
    }

    #[test]
    fn rejects_short_extension_lengths() {
        for length in 0..=3 {
            let mut data =
                block(0, &[], UF2Flags::EXTENSION_TAGS_PRESENT, None);
            data[UF2_HEADER_SIZE..UF2_HEADER_SIZE + 4].copy_from_slice(
                &((VERSION_EXTENSION_TAG << 8) | length).to_le_bytes(),
            );
            assert!(UF2RangeIterator::new(&data).is_err());
        }
    }

    #[test]
    fn rejects_extension_past_remaining_block_data() {
        let mut data = block(
            0,
            &[0; UF2_MAX_PAYLOAD_SIZE - 4],
            UF2Flags::EXTENSION_TAGS_PRESENT,
            None,
        );
        let extension = UF2_HEADER_SIZE + UF2_MAX_PAYLOAD_SIZE - 4;
        data[extension..extension + 4]
            .copy_from_slice(&((VERSION_EXTENSION_TAG << 8) | 8).to_le_bytes());
        assert!(UF2RangeIterator::new(&data).is_err());
    }

    #[test]
    fn rejects_truncated_nonzero_extension_header() {
        let mut data = block(
            0,
            &[0; UF2_MAX_PAYLOAD_SIZE - 3],
            UF2Flags::EXTENSION_TAGS_PRESENT,
            None,
        );
        data[UF2_HEADER_SIZE + UF2_MAX_PAYLOAD_SIZE - 3] = 1;
        assert!(UF2RangeIterator::new(&data).is_err());
    }

    #[test]
    fn rejects_empty_input() {
        assert!(UF2RangeIterator::new(&[]).is_err());
    }

    #[test]
    fn rejects_invalid_reboot_extension() {
        let mut data = block(
            0x20000000,
            &[1; 4],
            UF2Flags::NOT_MAIN_FLASH,
            Some(0x20000001),
        );
        let extension = UF2_HEADER_SIZE + 4;
        data[extension..extension + 4]
            .copy_from_slice(&((REBOOT_EXTENSION_TAG << 8) | 6).to_le_bytes());
        assert!(UF2RangeIterator::new(&data).is_err());
    }

    #[test]
    fn does_not_merge_ranges_across_address_overflow() {
        let data = [
            block(0xffffff00, &[1; 256], 0, None),
            block(0, &[2; 4], 0, None),
        ]
        .concat();
        let result = ranges(&data);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn skips_file_container_blocks() {
        assert!(
            ranges(&block(0, &[1; 4], UF2Flags::FILE_CONTAINER, None))
                .is_empty()
        );
    }
}
