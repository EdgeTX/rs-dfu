pub(crate) const DFU_DESC_TYPE: u8 = 0x21;
pub(crate) const DFU_DESC_LEN: usize = 9;

pub const DFUSE_VERSION_NUMBER: u16 = 0x11A;

/// DFU functional descriptor
///
/// Represents the DFU functional descriptor as described in section 4.1.3.
///
#[derive(Default)]
pub struct DfuDescriptor {
    attributes: u8,
    detach_timeout: u16,
    transfer_size: u16,
    dfu_version: u16,
}

impl DfuDescriptor {
    const BIT_CAN_DNLOAD: u8 = 1 << 0;
    const BIT_CAN_UPLOAD: u8 = 1 << 1;
    const BIT_MANIFESTATION_TOLERANT: u8 = 1 << 2;
    const BIT_WILL_DETACH: u8 = 1 << 3;

    pub(crate) fn new(raw_desc: &[u8]) -> Self {
        Self {
            attributes: raw_desc[2],
            detach_timeout: (raw_desc[4] as u16) << 8 | (raw_desc[3] as u16),
            transfer_size: (raw_desc[6] as u16) << 8 | (raw_desc[5] as u16),
            dfu_version: (raw_desc[8] as u16) << 8 | (raw_desc[7] as u16),
        }
    }

    /// Download capable (`bitCanDnload`)
    #[doc(alias = "bitCanDnload")]
    pub fn can_download(&self) -> bool {
        self.attributes & Self::BIT_CAN_DNLOAD != 0
    }

    /// Upload capable (`bitCanUpload`)
    #[doc(alias = "bitCanUpload")]
    pub fn can_upload(&self) -> bool {
        self.attributes & Self::BIT_CAN_UPLOAD != 0
    }

    /// Device is able to communicate via USB after
    /// Manifestation phase (`bitManifestationTolerant`)
    #[doc(alias = "bitManifestationTolerant")]
    pub fn manifestation_tolerant(&self) -> bool {
        self.attributes & Self::BIT_MANIFESTATION_TOLERANT != 0
    }

    /// Device will perform a bus detach-attach sequence when it receives
    /// a `DFU_DETACH` request (`bitWillDetach`). The host must not issue a USB Reset.
    #[doc(alias = "bitWillDetach")]
    pub fn will_detach(&self) -> bool {
        self.attributes & Self::BIT_WILL_DETACH != 0
    }

    /// Time, in milliseconds, that the device will wait after receipt of the `DFU_DETACH`
    /// request (`wDetachTimeOut`). If this time elapses without a USB reset,
    /// then the device will terminate the Reconfiguration phase and revert back
    /// to normal operation. This represents the maximum time that the device can wait
    /// (depending on its timers, etc.).
    /// The host may specify a shorter timeout in the `DFU_DETACH` request.
    #[doc(alias = "wDetachTimeout")]
    pub fn detach_timeout(&self) -> u16 {
        self.detach_timeout
    }

    /// Maximum number of bytes that the device can accept per control-write transaction
    /// (`wTransferSize`).
    #[doc(alias = "wTransferSize")]
    pub fn transfer_size(&self) -> u16 {
        self.transfer_size
    }

    /// Numeric expression identifying the version of the DFU Specification release
    /// (`bcdDFUVersion`).
    #[doc(alias = "bcdDFUVersion")]
    pub fn dfu_version(&self) -> u16 {
        self.dfu_version
    }
}
