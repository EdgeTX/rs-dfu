use std::time::{Duration, Instant};

use dfu::{DfuConnection, DfuDevice, DfuError, find_dfu_devices};

use crate::CliError;

pub(crate) fn reboot(
    addr: u32,
    device: DfuDevice,
    start_addr: Option<u32>,
) -> Result<(), CliError> {
    let dev_info = device.device_info();
    let connection = device.connect(0, 0)?;

    println!("Rebooting...");
    connection.reboot(addr, b"BDFU", start_addr.unwrap_or(0x08000000))?;
    drop(connection);

    println!("Reconnecting...");
    let start = Instant::now();
    let connection = reconnect(dev_info.vendor_id(), dev_info.product_id())?;
    let status = connection.get_status()?;
    println!("Reconnected in {:?}", start.elapsed());

    status.ok()?;
    Ok(())
}

fn reconnect(vid: u16, pid: u16) -> Result<DfuConnection, DfuError> {
    let start = Instant::now();
    loop {
        let devices = find_dfu_devices(Some(vid), Some(pid))?;
        if !devices.is_empty() {
            return devices[0].connect(0, 0);
        }
        if start.elapsed() >= Duration::from_secs(30) {
            return Err(DfuError::Timeout);
        }
    }
}
