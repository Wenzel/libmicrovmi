use crate::api::DriverType;
use std::error::Error;

#[derive(thiserror::Error, Debug)]
pub enum MicrovmiError {
    #[error("no suitable microvmi driver available")]
    NoDriverAvailable,
    #[error("driver {0:?} has not been compiled")]
    DriverNotCompiled(DriverType),
    #[error("VirtualBox VBoxVMI method needs a socket path (enable feature virtualbox-api for the VM's default socket)")]
    VBoxVmiSocketPathRequired,
    #[error("{driver:?} access method {method} has not been compiled (enable feature {feature})")]
    AccessMethodNotCompiled {
        driver: DriverType,
        method: &'static str,
        feature: &'static str,
    },
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error>,
    },
}
