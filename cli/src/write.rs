use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use dfu::{DfuDevice, DfuError, find_dfu_devices};
use uf2::{UF2RangeIterator, is_uf2_payload};

use crate::CliError;

pub(crate) fn download(
    data: &[u8],
    device: DfuDevice,
    start_address: Option<u32>,
) -> Result<(), CliError> {
    let mut device = device;
    reset_state(&device)?;
    if !is_uf2_payload(data) {
        download_range(data, &device, start_address)?;
    } else {
        for addr_range in UF2RangeIterator::new(data)? {
            if let Some(reboot_addr) = addr_range.reboot_address {
                device = reboot(
                    &device,
                    addr_range.start_address,
                    &addr_range.payload,
                    reboot_addr,
                )?;
            } else {
                download_range(
                    &addr_range.payload,
                    &device,
                    Some(addr_range.start_address),
                )?;
            }
        }
    }
    Ok(leave(&device)?)
}

pub(crate) fn reset_state(device: &DfuDevice) -> Result<(), DfuError> {
    println!("Resetting device state...");
    let connection = device.connect(0, 0)?;
    connection.reset_state()
}

fn download_range(
    data: &[u8],
    device: &DfuDevice,
    start_address: Option<u32>,
) -> Result<(), DfuError> {
    let start_address =
        start_address.unwrap_or(device.get_default_start_address());
    let end_address = start_address + (data.len() as u32) - 1;

    let intf = device.find_interface(start_address, Some(end_address))?;
    let connection = device.connect(intf.interface(), intf.alt_setting())?;

    // erase first
    let erase_pages = intf.get_erase_pages(start_address, end_address);
    let erase_start = erase_pages.first().unwrap_or(&0u32).to_owned();
    let pages = erase_pages.len();

    for (page, page_addr) in erase_pages.into_iter().enumerate() {
        print!(
            "\r  Erasing page {:2} of {:2} @ 0x{:08x}",
            page + 1,
            pages,
            erase_start
        );
        let _ = io::stdout().flush();
        if let Err(err) = connection.dfuse_page_erase(page_addr) {
            println!(" ❌");
            return Err(err);
        }
    }
    println!();

    let mut addr = start_address;
    let mut bytes_downloaded: usize = 0;
    let transfer_size = connection.transfer_size();

    for chunk in data.chunks(transfer_size as usize) {
        connection.download(addr, chunk)?;
        addr += chunk.len() as u32;
        bytes_downloaded += chunk.len();

        let percentage = (100 * bytes_downloaded) / data.len();
        let filled = (60 * bytes_downloaded) / data.len();
        print!(
            "\r  Flashing {:3}% [{}]",
            percentage,
            "#".repeat(filled) + &" ".repeat(60 - filled)
        );
        let _ = io::stdout().flush();
    }
    println!();

    Ok(())
}

fn reboot(
    device: &DfuDevice,
    addr: u32,
    payload: &[u8],
    reboot_addr: u32,
) -> Result<DfuDevice, DfuError> {
    let connection = device.connect(0, 0)?;
    connection.reboot(addr, payload, reboot_addr)?;
    drop(connection);

    println!("Waiting for device to reconnect...");
    let start = Instant::now();
    loop {
        let devices = find_dfu_devices(
            Some(device.vendor_id()),
            Some(device.product_id()),
        )?;
        if !devices.is_empty() {
            println!("Device reconnected");
            return Ok(devices.into_iter().next().unwrap());
        }
        if start.elapsed() >= Duration::from_secs(30) {
            return Err(DfuError::Timeout);
        }
    }
}

fn leave(device: &DfuDevice) -> Result<(), DfuError> {
    println!("Leaving DFU...");
    let connection = device.connect(0, 0)?;
    connection.leave()
}
