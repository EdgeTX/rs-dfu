use dfu::{DfuDevice, DfuMemSegment, find_dfu_devices};

use crate::CliError;

pub(crate) fn list_dfu_devices(
    vid: Option<u16>,
    pid: Option<u16>,
) -> Result<(), CliError> {
    let devices = find_dfu_devices(vid, pid)?;
    if devices.is_empty() {
        println!("No DFU device found");
    } else {
        print_devices(&devices);
    }
    Ok(())
}

fn print_segment(prefix: &str, segment: &DfuMemSegment) {
    let mut page_size = segment.page_size();
    let page_char = if page_size >= 1024 {
        page_size /= 1024;
        "K"
    } else {
        " "
    };
    println!(
        "{}0x{:08X} {:2} pages of {:4}{} bytes ({}{}{})",
        prefix,
        segment.start_addr(),
        segment.pages(),
        page_size,
        page_char,
        if segment.readable() { "r" } else { "" },
        if segment.writable() { "w" } else { "" },
        if segment.erasable() { "e" } else { "" },
    );
}

fn print_devices(devices: &Vec<DfuDevice>) {
    for device in devices {
        println!(
            "Bus {} Device {:03}: ID {:04x}:{:04x} (dfuse={})",
            device.bus_id(),
            device.device_address(),
            device.vendor_id(),
            device.product_id(),
            device.is_dfuse(),
        );

        for interface in device.interfaces() {
            let layout = interface.layout();
            println!(
                "  {} (intf={}, alt={}):",
                layout.name,
                interface.interface(),
                interface.alt_setting(),
            );
            for segment in &layout.segments {
                print_segment("    ", segment);
            }
        }
    }
}
