pub use iter::*;

mod iter;

pub const UF2_BLOCK_SIZE: usize = 512;
pub const UF2_HEADER_SIZE: usize = 32;
pub const UF2_MAX_PAYLOAD_SIZE: usize = UF2_BLOCK_SIZE - UF2_HEADER_SIZE;

pub const UF2_MAGIC_START1: u32 = 0x0a324655; // "UF2\n"
pub const UF2_MAGIC_START2: u32 = 0x9e5d5157; // Randomly selected
pub const UF2_MAGIC_FINAL: u32 = 0x0ab16f30;

pub const UF2_MAGIC_VALUES: &[(usize, u32)] = &[
    (0, UF2_MAGIC_START1),
    (4, UF2_MAGIC_START2),
    (UF2_BLOCK_SIZE - 4, UF2_MAGIC_FINAL),
];

// official extension tags
pub const VERSION_EXTENSION_TAG: u32 = 0x9fc7bc;
pub const DEVICE_EXTENSION_TAG: u32 = 0x650d9d;

// EdgeTX specific extension
pub const REBOOT_EXTENSION_TAG: u32 = 0xe60835;

pub struct UF2Flags(u32);

pub struct UF2BlockData {
    pub flags: UF2Flags,
    pub flash_address: u32,
    pub block_nr: u32,
    pub total_blocks: u32,
    pub file_size: u32, // or board family ID
    pub payload: Vec<u8>,
    pub extensions: Vec<UF2Extension>,
}

pub struct UF2Extension {
    pub tag: u32,
    pub payload: Vec<u8>,
}

pub struct UF2DecodeError {
    pub err: String,
}

impl UF2Flags {
    pub const NOT_MAIN_FLASH: u32 = 0x00000001;
    pub const FILE_CONTAINER: u32 = 0x00001000;
    pub const FAMILY_ID_PRESENT: u32 = 0x00002000;
    pub const MD5_CHECKSUM_PRESENT: u32 = 0x00004000;
    pub const EXTENSION_TAGS_PRESENT: u32 = 0x00008000;

    pub fn is_main_flash(&self) -> bool {
        self.0 & Self::NOT_MAIN_FLASH == 0
    }

    pub fn file_container(&self) -> bool {
        self.0 & Self::FILE_CONTAINER != 0
    }

    pub fn family_id_present(&self) -> bool {
        self.0 & Self::FAMILY_ID_PRESENT != 0
    }

    pub fn md5_checksum_present(&self) -> bool {
        self.0 & Self::MD5_CHECKSUM_PRESENT != 0
    }

    pub fn extension_tags_present(&self) -> bool {
        self.0 & Self::EXTENSION_TAGS_PRESENT != 0
    }
}

impl UF2BlockData {
    pub fn decode(data: &[u8]) -> Result<UF2BlockData, UF2DecodeError> {
        if !is_uf2_block(data) {
            return Err(UF2DecodeError::new(
                "magic values check failed".to_string(),
            ));
        }

        let flags = extract_u32(data, 8);
        let payload_size = extract_u32(data, 16) as usize;

        if payload_size > UF2_MAX_PAYLOAD_SIZE {
            return Err(UF2DecodeError::new(
                format!("payload size too big ({payload_size})").to_string(),
            ));
        }

        let payload = &data[UF2_HEADER_SIZE..(UF2_HEADER_SIZE + payload_size)];
        let extension_payload =
            &data[(UF2_HEADER_SIZE + payload_size)..(data.len() - 4)];

        Ok(UF2BlockData {
            flags: UF2Flags(flags),
            flash_address: extract_u32(data, 12),
            block_nr: extract_u32(data, 20),
            total_blocks: extract_u32(data, 24),
            file_size: extract_u32(data, 28),
            payload: Vec::from(payload),
            extensions: decode_extensions(UF2Flags(flags), extension_payload),
        })
    }

    pub fn file_size(&self) -> Option<u32> {
        if !self.flags.family_id_present() {
            Some(self.file_size)
        } else {
            None
        }
    }

    pub fn family_id(&self) -> Option<u32> {
        if self.flags.family_id_present() {
            Some(self.file_size)
        } else {
            None
        }
    }

    pub fn is_reboot_block(&self) -> bool {
        !self.flags.is_main_flash()
            && self
                .extensions
                .iter()
                .any(|ext| ext.tag == REBOOT_EXTENSION_TAG)
    }

    pub fn get_device_description(&self) -> Option<String> {
        self.get_extension_string(DEVICE_EXTENSION_TAG)
            .map(String::from)
    }

    pub fn get_version_description(&self) -> Option<String> {
        self.get_extension_string(VERSION_EXTENSION_TAG)
            .map(String::from)
    }

    pub fn get_reboot_address(&self) -> Option<u32> {
        let ext = self.get_extension(REBOOT_EXTENSION_TAG)?;
        if ext.payload.len() == 4 {
            let addr = u32::from_le_bytes(ext.payload[..].try_into().unwrap());
            Some(addr)
        } else {
            None
        }
    }

    fn get_extension_string(&self, tag: u32) -> Option<&str> {
        match self.get_extension(tag) {
            Some(ext) => str::from_utf8(&ext.payload).ok(),
            None => None,
        }
    }

    fn get_extension(&self, tag: u32) -> Option<&UF2Extension> {
        self.extensions.iter().find(|ext| ext.tag == tag)
    }
}

impl UF2DecodeError {
    pub fn new(err: String) -> Self {
        UF2DecodeError { err }
    }
}

impl std::fmt::Display for UF2DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UF2 decode error: {}", self.err)
    }
}

pub fn is_uf2_payload(data: &[u8]) -> bool {
    check_magic(&UF2_MAGIC_VALUES[0..1], data)
}

pub fn is_uf2_block(data: &[u8]) -> bool {
    check_magic(UF2_MAGIC_VALUES, data)
}

fn check_magic(magics: &[(usize, u32)], data: &[u8]) -> bool {
    magics.iter().all(|(offset, magic)| {
        (data.len() >= offset + 4) && (*magic == extract_u32(data, *offset))
    })
}

fn pad32(n: usize) -> usize {
    let rem = n % 4;
    if rem > 0 { n + 4 - rem } else { n }
}

fn extract_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..(offset + 4)].try_into().unwrap())
}

fn decode_extensions(flags: UF2Flags, data: &[u8]) -> Vec<UF2Extension> {
    let mut offset = 0;
    let mut extensions: Vec<UF2Extension> = Vec::new();

    if flags.extension_tags_present() {
        while offset < data.len() {
            let hdr = extract_u32(data, offset);
            if hdr == 0 {
                break;
            }

            let length = hdr & 0xff;
            let tag = (hdr >> 8) & 0xffffff;

            extensions.push(UF2Extension {
                tag,
                payload: Vec::from(
                    &data[(offset + 4)..(offset + (length as usize))],
                ),
            });

            offset += pad32(length as usize);
        }
    }

    extensions
}
