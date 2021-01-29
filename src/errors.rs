use crate::api::DriverType;
use thiserror::Error;

#[derive(Error, Debug)]
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
