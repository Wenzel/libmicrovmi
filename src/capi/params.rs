use crate::api::params::{CommonInitParams, DriverInitParams, KVMInitParams};
use std::convert::TryFrom;
use std::ffi::{CStr, IntoStringError};
use std::os::raw::c_char;

/// equivalent of `CommonInitParams` with C compatibility
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CommonInitParamsFFI {
    pub vm_name: *const c_char,
}

/// equivalent of `KVMInitParams` with C compatibility
#[repr(C)]
#[derive(Debug, Clone)]
pub enum KVMInitParamsFFI {
    UnixSocket { path: *const c_char },
}

/// equivalent of `DriverInitParam` with C API compatibility
#[repr(C)]
#[derive(Debug, Clone)]
pub struct DriverInitParamsFFI {
    pub common: CommonInitParamsFFI,
    pub kvm: KVMInitParamsFFI,
}

// convert from FFI type to Rust type
impl TryFrom<DriverInitParamsFFI> for DriverInitParams {
    type Error = IntoStringError;

    fn try_from(value: DriverInitParamsFFI) -> Result<Self, Self::Error> {
        // build common params
        let vm_name = if value.common.vm_name.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(value.common.vm_name) }
                    .to_owned()
                    .into_string()?,
            )
        };
        let common = vm_name.map(|v| CommonInitParams { vm_name: v });
        // build kvm params
        let kvm_socket = match value.kvm {
            KVMInitParamsFFI::UnixSocket { path } => {
                if path.is_null() {
                    None
                } else {
                    Some(unsafe { CStr::from_ptr(path) }.to_owned().into_string()?)
                }
            }
        };
        let kvm = kvm_socket.map(|v| KVMInitParams::UnixSocket { path: v });
        Ok(DriverInitParams {
            common,
            kvm,
            ..Default::default()
        })
    }
}
