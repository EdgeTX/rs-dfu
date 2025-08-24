use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use clap_num::maybe_hex;
use parse_size::parse_size;

use dfu::{DfuDevice, find_dfu_devices};
use error::CliError;
use list::*;
use read::*;
use reboot::*;
use uf2::*;
use write::*;

mod error;
mod list;
mod read;
mod reboot;
mod write;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// list DFU devices
    List {
        /// vendor ID (ex: "0483")
        #[clap(short, long, value_parser=hex_u16)]
        vendor: Option<u16>,
        /// product ID (ex: "df11")
        #[clap(short, long, value_parser=hex_u16)]
        product: Option<u16>,
    },
    /// read from device
    Read {
        /// file to write (either raw binary or UF2)
        file: PathBuf,
        /// vendor ID (ex: "0483")
        #[clap(short, long, value_parser=hex_u16)]
        vendor: Option<u16>,
        /// product ID (ex: "df11")
        #[clap(short, long, value_parser=hex_u16)]
        product: Option<u16>,
        /// start address (ex: 0x0800000)
        #[clap(short, long, value_parser=maybe_hex::<u32>)]
        start_address: Option<u32>,
        /// length (ex: 64K, 2MB)
        #[clap(short, long, value_parser=parse_length)]
        length: Option<u32>,
    },
    /// write to device
    Write {
        /// file to write (either raw binary or UF2)
        file: PathBuf,
        /// vendor ID (ex: "0483")
        #[clap(short, long, value_parser=hex_u16)]
        vendor: Option<u16>,
        /// product ID (ex: "df11")
        #[clap(short, long, value_parser=hex_u16)]
        product: Option<u16>,
        /// start address (ex: 0x0800000)
        #[clap(short, long, value_parser=maybe_hex::<u32>)]
        start_address: Option<u32>,
    },
    /// reboot into EdgeTX DFU bootloader
    Reboot {
        /// reboot tag address
        #[clap(value_parser=maybe_hex::<u32>)]
        address: u32,
        /// vendor ID (ex: "0483")
        #[clap(short, long, value_parser=hex_u16)]
        vendor: Option<u16>,
        /// product ID (ex: "df11")
        #[clap(short, long, value_parser=hex_u16)]
        product: Option<u16>,
        /// start address (ex: 0x0800000)
        #[clap(short, long, value_parser=maybe_hex::<u32>)]
        start_address: Option<u32>,
    },
    /// inspect UF2 file
    Uf2 {
        /// UF2 file
        file: PathBuf,
    },
}

impl Default for Commands {
    fn default() -> Self {
        Commands::List {
            vendor: None,
            product: None,
        }
    }
}

fn hex_u16(s: &str) -> Result<u16, String> {
    <u16>::from_str_radix(s, 16).map_err(|e| format!("{e}"))
}

fn parse_length(s: &str) -> Result<u32, String> {
    let len = parse_size(s).map_err(|e| format!("{e}"))?;
    len.try_into().map_err(|e| format!("{e}"))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    env_logger::init();

    if let Err(err) = match &cli.command.unwrap_or_default() {
        Commands::List { vendor, product } => {
            list_dfu_devices(*vendor, *product)
        }
        Commands::Read {
            file,
            vendor,
            product,
            start_address,
            length,
        } => read_file(file, vendor, product, start_address, length),
        Commands::Write {
            file,
            vendor,
            product,
            start_address,
        } => write_file(file, vendor, product, start_address),
        Commands::Reboot {
            address,
            vendor,
            product,
            start_address,
        } => reboot_cmd(address, vendor, product, start_address),
        Commands::Uf2 { file } => show_uf2(file),
    } {
        eprintln!("Error: {err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn get_dfu_device(
    vid: &Option<u16>,
    pid: &Option<u16>,
) -> Result<DfuDevice, CliError> {
    let devices = find_dfu_devices(*vid, *pid)?;
    if devices.is_empty() {
        return Err(CliError::NoDFUDevice);
    }

    if devices.len() > 1 {
        return Err(CliError::ManyDFUDevices);
    }

    Ok(devices.into_iter().next().unwrap())
}

fn read_file(
    file: &PathBuf,
    vid: &Option<u16>,
    pid: &Option<u16>,
    start_address: &Option<u32>,
    length: &Option<u32>,
) -> Result<(), CliError> {
    let device = get_dfu_device(vid, pid)?;
    let data = upload(device, *start_address, *length)?;
    fs::write(file, data)?;
    Ok(())
}

fn write_file(
    file: &PathBuf,
    vid: &Option<u16>,
    pid: &Option<u16>,
    start_address: &Option<u32>,
) -> Result<(), CliError> {
    let device = get_dfu_device(vid, pid)?;
    let data = fs::read(file)?;
    download(&data, device, *start_address)?;
    Ok(())
}

fn reboot_cmd(
    address: &u32,
    vid: &Option<u16>,
    pid: &Option<u16>,
    start_address: &Option<u32>,
) -> Result<(), CliError> {
    let device = get_dfu_device(vid, pid)?;
    reboot(*address, device, *start_address)?;
    Ok(())
}

fn show_uf2(file: &PathBuf) -> Result<(), CliError> {
    let data = fs::read(file)?;
    if !is_uf2_block(&data) {
        return Err(CliError::UF2(UF2DecodeError::new(
            "invalid first block".to_string(),
        )));
    }
    let block = UF2BlockData::decode(&data[0..UF2_BLOCK_SIZE])?;
    println!(
        "Device: {}",
        block.get_device_description().unwrap_or_default()
    );
    println!(
        "Version: {}",
        block.get_version_description().unwrap_or_default()
    );

    println!("Parts:");
    for addr_range in UF2RangeIterator::new(&data)? {
        println!(
            "  - 0x{:08x}: {:7} bytes{}",
            addr_range.start_address,
            addr_range.payload.len(),
            if let Some(addr) = &addr_range.reboot_address {
                format!(" (reboot @ 0x{:08x})", *addr)
            } else {
                "".into()
            }
        );
    }
    Ok(())
}
