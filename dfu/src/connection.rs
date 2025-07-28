use std::time::Instant;

use nusb::{
    MaybeFuture,
    transfer::{ControlIn, ControlOut, ControlType, Recipient},
};

use crate::DEFAULT_TIMEOUT;
use crate::error::*;

const DFU_CMD_DOWNLOAD: u8 = 1;
const DFU_CMD_UPLOAD: u8 = 2;
const DFU_CMD_GETSTATUS: u8 = 3;
const DFU_CMD_CLRSTATUS: u8 = 4;
// const DFU_CMD_GETSTATE: u8 = 5;
const DFU_CMD_ABORT: u8 = 6;

const DFU_STATE_LEN: u16 = 6;

const DFUSE_CMD_ADDR: u8 = 0x21;
const DFUSE_CMD_ERASE: u8 = 0x41;

// const DFU_STATE_APP_IDLE: u8 = 0x00;
// const DFU_STATE_APP_DETACH: u8 = 0x01;
const DFU_STATE_DFU_IDLE: u8 = 0x02;
// const DFU_STATE_DFU_DOWNLOAD_SYNC: u8 = 0x03;
// const DFU_STATE_DFU_DOWNLOAD_BUSY: u8 = 0x04;
const DFU_STATE_DFU_DOWNLOAD_IDLE: u8 = 0x05;
// const DFU_STATE_DFU_MANIFEST_SYNC: u8 = 0x06;
// const DFU_STATE_DFU_MANIFEST: u8 = 0x07;
// const DFU_STATE_DFU_MANIFEST_WAIT_RESET: u8 = 0x08;
// const DFU_STATE_DFU_UPLOAD_IDLE: u8 = 0x09;
// const DFU_STATE_DFU_ERROR: u8 = 0x0a;

pub struct DfuConnection {
    interface: nusb::Interface,
    xfer_size: u16,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct DfuStatus {
    status: u8,
    poll_timeout: u32,
    state: u8,
}

impl DfuStatus {
    fn from_raw(data: &[u8]) -> Self {
        DfuStatus {
            status: data[0],
            poll_timeout: (data[3] as u32) << 16
                | (data[2] as u32) << 8
                | (data[1] as u32),
            state: data[4],
        }
    }

    pub fn ok(&self) -> Result<(), DfuError> {
        self.ret(())
    }

    pub fn ret<T>(&self, t: T) -> Result<T, DfuError> {
        if self.status != 0 {
            Err(DfuError::from(self))
        } else {
            Ok(t)
        }
    }
}

impl From<&DfuStatus> for DfuError {
    fn from(st: &DfuStatus) -> Self {
        DfuError::Status(st.status)
    }
}

impl DfuConnection {
    pub(crate) fn new(interface: nusb::Interface, xfer_size: u16) -> Self {
        DfuConnection {
            interface,
            xfer_size: if xfer_size > 0 {
                xfer_size
            } else {
                crate::DEFAULT_TRANSFER_SIZE
            },
        }
    }

    pub fn transfer_size(&self) -> u16 {
        self.xfer_size
    }

    pub fn reset_state(&self) -> Result<(), DfuError> {
        let mut st = self.get_status()?;
        if st.status != 0 {
            self.clear_status()?;
            st = self.get_status()?;
        }
        if st.state != DFU_STATE_DFU_IDLE {
            self.abort()?;
            st = self.get_status()?;
        }
        st.ok()
    }

    pub fn get_status(&self) -> Result<DfuStatus, DfuError> {
        let data = self.dfu_cmd_in(DFU_CMD_GETSTATUS, 0, DFU_STATE_LEN)?;
        let st = DfuStatus::from_raw(&data);
        Ok(st)
    }

    pub fn clear_status(&self) -> Result<(), DfuError> {
        self.dfu_cmd_out(DFU_CMD_CLRSTATUS, 0, &[])
    }

    pub fn abort(&self) -> Result<(), DfuError> {
        self.dfu_cmd_out(DFU_CMD_ABORT, 0, &[])
    }

    pub fn download(&self, addr: u32, data: &[u8]) -> Result<(), DfuError> {
        self.dfuse_set_address(addr)?;
        self.dfu_dnload(2, data)
    }

    pub fn upload(
        &self,
        block_nr: u16,
        length: u16,
    ) -> Result<Vec<u8>, DfuError> {
        self.dfu_upload(2 + block_nr, length)
    }

    pub fn reboot(
        &self,
        addr: u32,
        data: &[u8],
        reboot_addr: u32,
    ) -> Result<(), DfuError> {
        self.download(addr, data)?;
        self.dfuse_set_address(reboot_addr)?;
        let _ = self.dfu_dnload(0, &[]);
        Ok(())
    }

    pub fn leave(&self) -> Result<(), DfuError> {
        let _ = self.dfu_dnload(0, &[]);
        Ok(())
    }

    pub fn dfuse_page_erase(&self, addr: u32) -> Result<(), DfuError> {
        let erase_cmd: Vec<u8> = vec![
            DFUSE_CMD_ERASE,
            addr as u8,
            (addr >> 8) as u8,
            (addr >> 16) as u8,
            (addr >> 24) as u8,
        ];
        self.dfu_dnload(0, &erase_cmd)
    }

    pub fn dfuse_leave(&self, addr: u32) -> Result<(), DfuError> {
        self.dfuse_set_address(addr)?;
        self.dfu_dnload(0, &[])
    }

    pub fn dfuse_set_address(&self, addr: u32) -> Result<(), DfuError> {
        let addr_cmd: Vec<u8> = vec![
            DFUSE_CMD_ADDR,
            addr as u8,
            (addr >> 8) as u8,
            (addr >> 16) as u8,
            (addr >> 24) as u8,
        ];
        self.dfu_dnload(0, &addr_cmd)
    }

    fn dfu_dnload(
        &self,
        transaction: u16,
        data: &[u8],
    ) -> Result<(), DfuError> {
        self.dfu_cmd_out(DFU_CMD_DOWNLOAD, transaction, data)?;
        self.poll_until_idle()
    }

    fn dfu_upload(
        &self,
        transaction: u16,
        length: u16,
    ) -> Result<Vec<u8>, DfuError> {
        self.dfu_cmd_in(DFU_CMD_UPLOAD, transaction, length)
    }

    fn poll_until_idle(&self) -> Result<(), DfuError> {
        let start = Instant::now();
        loop {
            let st = self.get_status()?;
            if st.state == DFU_STATE_DFU_DOWNLOAD_IDLE {
                return st.ok();
            }
            if start.elapsed() >= DEFAULT_TIMEOUT * 2 {
                return Err(DfuError::Timeout);
            }
        }
    }

    fn dfu_cmd_out(
        &self,
        req: u8,
        value: u16,
        data: &[u8],
    ) -> Result<(), DfuError> {
        let index = self.interface.interface_number() as u16;
        Ok(self
            .interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: req,
                    value,
                    index,
                    data,
                },
                DEFAULT_TIMEOUT,
            )
            .wait()?)
    }

    fn dfu_cmd_in(
        &self,
        req: u8,
        value: u16,
        length: u16,
    ) -> Result<Vec<u8>, DfuError> {
        let index = self.interface.interface_number() as u16;
        Ok(self
            .interface
            .control_in(
                ControlIn {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: req,
                    value,
                    index,
                    length,
                },
                DEFAULT_TIMEOUT,
            )
            .wait()?)
    }
}
