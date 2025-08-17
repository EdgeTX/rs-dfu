use std::{
    cmp,
    io::{self, Write},
};

use dfu::{DfuDevice, DfuError};

use crate::CliError;

pub(crate) fn upload(
    device: DfuDevice,
    start_address: Option<u32>,
    length: Option<u32>,
) -> Result<Vec<u8>, CliError> {
    let start_address =
        start_address.unwrap_or(device.get_default_start_address());
    let end_address = length.map(|l| start_address + l - 1);

    let intf = device.find_interface(start_address, end_address)?;
    let segments = intf.find_segments(start_address, end_address);
    if segments.is_empty() {
        return Err(CliError::Dfu(DfuError::NoMemorySegments));
    }

    let end_address =
        end_address.unwrap_or(segments.last().unwrap().end_addr() - 1);

    let connection = device.connect(intf.interface(), intf.alt_setting())?;
    let transfer_size = connection.transfer_size() as u32;

    println!("Reseting state...");
    connection.reset_state()?;

    println!("Setting start address ({start_address:#010x})...");
    connection.dfuse_set_address(start_address)?;
    connection.reset_state()?;

    let total = end_address + 1 - start_address;
    let mut bytes_uploaded: u32 = 0;
    let mut block_nr: u16 = 0;

    let mut data: Vec<u8> = Vec::new();
    while total - bytes_uploaded > 0 {
        let single_xfer_size = cmp::min(total - bytes_uploaded, transfer_size);
        data.extend(connection.upload(block_nr, single_xfer_size as u16)?);
        bytes_uploaded += single_xfer_size;
        block_nr += 1;

        let percentage = (100 * bytes_uploaded) / total;
        let filled = ((60 * bytes_uploaded) / total) as usize;
        print!(
            "\r  Reading {:3}% [{}]",
            percentage,
            "#".repeat(filled) + &" ".repeat(60 - filled)
        );
        let _ = io::stdout().flush();
    }
    println!();

    Ok(data)
}
