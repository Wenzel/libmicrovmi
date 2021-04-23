//! This module defines the library error types

use crate::api::DriverType;
use std::error::Error;

#[derive(thiserror::Error, Debug)]
pub enum MicrovmiError {
    #[error("no suitable microvmi driver available")]
    NoDriverAvailable,
    #[error("driver {0:?} has not been compiled")]
    DriverNotCompiled(DriverType),
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error>,
    },
}
