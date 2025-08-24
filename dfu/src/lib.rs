//! USB Device Firmware Upgrade (DFU) implementation based on [`nusb`]
//!
//! Provides a portable implementation of the DFU protocol with STM32
//! extensions (aka "DfuSe"). The main goal is to provide a library and command line tool
//! to upgrade [EdgeTX] firmware on compatible devices.
//!
//! Useful references:
//! - DFU: [USB Device Firmware Upgrade Specification, Revision 1.1](https://www.usb.org/sites/default/files/DFU_1.1.pdf)
//! - DfuSe: [STMicroelectronics AN3156](https://www.st.com/resource/en/application_note/an3156-usb-dfu-protocol-used-in-the-stm32-bootloader-stmicroelectronics.pdf)
//!
//! # Example
//!
//! The following example shows how to obtain a `Vec` of [DfuDevice]:
//! ```
//! use dfu::find_dfu_devices;
//!
//! match find_dfu_devices(None, None) {
//!     Ok(devices) => {
//!         if devices.is_empty() {
//!             println!("No DFU devices found");
//!         } else {
//!             println!("Found {} DFU devices", devices.len());
//!         }
//!     }
//!     Err(e) => println!("Error: {e}"),
//! }
//! ```
//!
//!
//! [`nusb`]: https://docs.rs/nusb
//! [EdgeTX]: https://github.com/EdgeTX/edgetx

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_millis(5000u64);
pub(crate) const DEFAULT_TRANSFER_SIZE: u16 = 1024 * 2;

mod connection;
mod descriptor;
mod device;
mod error;
mod interface;
mod memory;

use std::time::Duration;

// Re-exports
pub use connection::DfuConnection;
pub use descriptor::{DFUSE_VERSION_NUMBER, DfuDescriptor};
pub use device::{DfuDevice, find_dfu_devices};
pub use error::DfuError;
pub use interface::DfuInterface;
pub use memory::{DfuMemSegment, DfuMemory};
