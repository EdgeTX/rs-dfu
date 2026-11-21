use std::time::Instant;

use dfu::DfuDevice;

use crate::{CliError, RECONNECT_TIMEOUT};

pub(crate) fn reboot(
    addr: u32,
    device: DfuDevice,
    start_addr: Option<u32>,
) -> Result<(), CliError> {
    let connection = device.connect(0, 0)?;

    println!("Rebooting...");
    connection.reboot(addr, b"BDFU", start_addr.unwrap_or(0x08000000))?;
    drop(connection);

    println!("Reconnecting...");
    let start = Instant::now();
    let connection = device
        .wait_for_reconnect(RECONNECT_TIMEOUT)?
        .connect(0, 0)?;
    let status = connection.get_status()?;
    println!("Reconnected in {:?}", start.elapsed());

    status.ok()?;
    Ok(())
}
