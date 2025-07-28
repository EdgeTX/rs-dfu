use std::cmp;

use nonempty::NonEmpty;
use regex::Regex;

#[derive(Debug, PartialEq, Eq)]
pub struct DfuMemory {
    pub name: String,
    pub segments: NonEmpty<DfuMemSegment>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DfuMemSegment {
    start_addr: u32,
    end_addr: u32,
    page_size: u32,
    mem_type: u8,
}

impl DfuMemory {
    pub fn find_segments(
        &self,
        start_address: u32,
        end_address: u32,
    ) -> Vec<DfuMemSegment> {
        self.segments
            .iter()
            .filter_map(|s| {
                if s.contains(start_address)
                    || s.contains(end_address)
                    || s.is_contained_in(start_address, end_address)
                {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl DfuMemSegment {
    pub fn start_addr(&self) -> u32 {
        self.start_addr
    }
    pub fn end_addr(&self) -> u32 {
        self.end_addr
    }
    pub fn page_size(&self) -> u32 {
        self.page_size
    }
    pub fn pages(&self) -> u32 {
        (self.end_addr - self.start_addr) / self.page_size
    }
    pub fn is_contained_in(&self, start_addr: u32, end_addr: u32) -> bool {
        start_addr <= self.start_addr && self.end_addr <= end_addr
    }
    pub fn contains(&self, addr: u32) -> bool {
        addr >= self.start_addr && addr <= self.end_addr
    }
    pub fn get_erase_pages(
        &self,
        start_addr: u32,
        end_addr: u32,
    ) -> (u32, u32) {
        let erase_start = cmp::max(start_addr, self.start_addr);
        let erase_end = cmp::min(end_addr, self.end_addr);
        (
            erase_start,
            (erase_end - erase_start).div_ceil(self.page_size()),
        )
    }
    pub fn readable(&self) -> bool {
        self.mem_type & 1 == 1
    }
    pub fn erasable(&self) -> bool {
        self.mem_type & 2 == 2
    }
    pub fn writable(&self) -> bool {
        self.mem_type & 4 == 4
    }
}

pub(crate) fn parse_memory_layout(mem_layout_str: &str) -> Option<DfuMemory> {
    let r = Regex::new(r"@?([^/]*?)\s*/0x([\da-fA-F]+)U?/(.*)").unwrap();
    let captures = r.captures(mem_layout_str)?;

    let name = String::from(&captures[1]);
    let start_addr = u32::from_str_radix(&captures[2], 16).unwrap_or(0);

    let segments = &captures[3];
    let sr = Regex::new(r"(\d+)\*(\d+)([KMB ])([a-g])(?:,|$)").unwrap();

    let mut layout = Vec::new();
    let mut current_addr = start_addr;

    for seg_match in sr.captures_iter(segments) {
        let pages: u32 = seg_match[1].parse().unwrap_or(0);
        let mut page_size: u32 = seg_match[2].parse().unwrap_or(0);
        let page_mul = &seg_match[3];

        match page_mul {
            "K" => page_size *= 1024,
            "M" => page_size *= 1024 * 1024,
            "B" => page_size *= 1,
            _ => {}
        }

        let memtype = (seg_match[4].chars().next().unwrap_or('a') as u8) & 7;
        let end_addr = current_addr + pages * page_size;

        let segment = DfuMemSegment {
            start_addr: current_addr,
            end_addr,
            page_size,
            mem_type: memtype,
        };

        current_addr = end_addr;
        layout.push(segment);
    }

    NonEmpty::from_vec(layout).map(|segments| DfuMemory { name, segments })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nonempty::nonempty;

    #[test]
    fn test_memory_layout() {
        let layout =
            parse_memory_layout("@Option Bytes   /0x5200201C/01*128 e");
        assert_eq!(
            layout,
            Some(DfuMemory {
                name: "Option Bytes".into(),
                segments: nonempty![DfuMemSegment {
                    start_addr: 0x5200201C,
                    end_addr: 0x5200201C + 128,
                    page_size: 128,
                    mem_type: b'e' & 7
                }],
            })
        );

        let layout =
            parse_memory_layout("@Internal Flash   /0x08000000/8*08Kg");
        assert_eq!(
            layout,
            Some(DfuMemory {
                name: "Internal Flash".into(),
                segments: nonempty![DfuMemSegment {
                    start_addr: 0x08000000,
                    end_addr: 0x08000000 + 64 * 1024,
                    page_size: 8 * 1024,
                    mem_type: b'g' & 7
                }],
            })
        );
    }
}
