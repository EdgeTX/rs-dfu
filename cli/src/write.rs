use std::{
    collections::HashSet,
    io::{self, Write},
};

use dfu::{DfuDevice, DfuError};
use uf2::{UF2RangeIterator, is_uf2_payload};

use crate::{CliError, RECONNECT_TIMEOUT};

pub(crate) fn download(
    data: &[u8],
    device: DfuDevice,
    start_address: Option<u32>,
) -> Result<(), CliError> {
    let mut device = device;
    // Pages erased so far: ranges sharing a page must not erase it again,
    // as this would wipe data written by a previous range.
    let mut erased_pages = HashSet::new();
    reset_state(&device)?;
    if !is_uf2_payload(data) {
        download_range(data, &device, start_address, &mut erased_pages)?;
    } else {
        for addr_range in UF2RangeIterator::new(data)? {
            if let Some(reboot_addr) = addr_range.reboot_address {
                device = reboot(
                    &device,
                    addr_range.start_address,
                    &addr_range.payload,
                    reboot_addr,
                )?;
                erased_pages.clear();
            } else {
                download_range(
                    &addr_range.payload,
                    &device,
                    Some(addr_range.start_address),
                    &mut erased_pages,
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
    erased_pages: &mut HashSet<u32>,
) -> Result<(), DfuError> {
    if data.is_empty() {
        return Ok(());
    }
    let start_address =
        start_address.unwrap_or(device.get_default_start_address());
    let end_address = start_address + (data.len() as u32) - 1;

    let intf = device.find_interface(start_address, Some(end_address))?;
    let connection = device.connect(intf.interface(), intf.alt_setting())?;

    // erase first
    let erase_pages: Vec<u32> = intf
        .get_erase_pages(start_address, end_address)
        .into_iter()
        .filter(|page| !erased_pages.contains(page))
        .collect();
    let pages = erase_pages.len();

    for (page, page_addr) in erase_pages.into_iter().enumerate() {
        print!(
            "\r  Erasing page {:2} of {:2} @ 0x{:08x}",
            page + 1,
            pages,
            page_addr,
        );
        let _ = io::stdout().flush();
        if let Err(err) = connection.dfuse_page_erase(page_addr) {
            println!(" ❌");
            return Err(err);
        }
        erased_pages.insert(page_addr);
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
    let device = device.wait_for_reconnect(RECONNECT_TIMEOUT)?;
    println!("Device reconnected");
    Ok(device)
}

fn leave(device: &DfuDevice) -> Result<(), DfuError> {
    println!("Leaving DFU...");
    let connection = device.connect(0, 0)?;
    connection.leave()
}
