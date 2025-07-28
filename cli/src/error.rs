use std::{fmt::Display, io};

use ::uf2::UF2DecodeError;
use dfu::DfuError;

pub enum CliError {
    IO(io::Error),
    Dfu(DfuError),
    UF2(UF2DecodeError),
    NoDFUDevice,
    ManyDFUDevices,
}

impl From<io::Error> for CliError {
    fn from(value: io::Error) -> Self {
        CliError::IO(value)
    }
}

impl From<DfuError> for CliError {
    fn from(value: DfuError) -> Self {
        CliError::Dfu(value)
    }
}

impl From<UF2DecodeError> for CliError {
    fn from(value: UF2DecodeError) -> Self {
        CliError::UF2(value)
    }
}

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::IO(err) => write!(f, "IO error: {err}"),
            CliError::Dfu(err) => write!(f, "DFU error: {err}"),
            CliError::UF2(err) => write!(f, "{err}"),
            CliError::NoDFUDevice => write!(f, "No DFU device"),
            CliError::ManyDFUDevices => write!(f, "More than one DFU devices"),
        }
    }
}
