#[derive(Debug)]
pub enum DfuError {
    Usb(nusb::Error),
    Transfer(nusb::transfer::TransferError),
    Status(u8),
    UnalignedAddress,
    InvalidInterface,
    NoMemorySegments,
    Timeout,
}

impl std::error::Error for DfuError {}

impl std::fmt::Display for DfuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DfuError::Usb(err) => write!(f, "USB error: {}", err),
            DfuError::Transfer(err) => write!(f, "Transfer error: {}", err),
            DfuError::Status(code) => {
                write!(f, "DFU status error: code {}", code)
            }
            DfuError::UnalignedAddress => {
                write!(f, "Unaligned page address")
            }
            DfuError::InvalidInterface => {
                write!(f, "Invalid interface")
            }
            DfuError::NoMemorySegments => {
                write!(f, "No compatible memory segments")
            }
            DfuError::Timeout => {
                write!(f, "Timeout")
            }
        }
    }
}

impl From<nusb::Error> for DfuError {
    fn from(err: nusb::Error) -> Self {
        DfuError::Usb(err)
    }
}

impl From<nusb::transfer::TransferError> for DfuError {
    fn from(err: nusb::transfer::TransferError) -> Self {
        DfuError::Transfer(err)
    }
}
