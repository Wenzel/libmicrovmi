use crate::api::params::{
    CommonInitParams, DriverInitParams, KVMInitParams, MemflowConnectorParams, MemflowInitParams,
};
use std::convert::TryFrom;
use std::ffi::{CStr, IntoStringError};
use std::os::raw::c_char;

/// equivalent of `CommonInitParams` with C compatibility
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CommonInitParamsFFI {
    pub vm_name: *mut c_char,
}

/// equivalent of `KVMInitParams` with C compatibility
#[repr(C)]
#[derive(Debug, Clone)]
pub enum KVMInitParamsFFI {
    UnixSocket { path: *mut c_char },
}

/// equivalent of `MemflowConnectorParams`with C compatiblity
#[repr(C)]
#[derive(Debug, Clone)]
pub enum MemflowConnectorParamsFFI {
    Default {
        args_arr: *mut *mut c_char,
        args_arr_len: usize,
    },
}

/// equivalent of `MemflowInitParams` with C compatibility
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MemflowInitParamsFFI {
    /// connector name
    pub connector_name: *mut c_char,
    /// optional connector initialization parameters
    pub connector_args: MemflowConnectorParamsFFI,
}

/// equivalent of `DriverInitParam` with C compatibility
#[repr(C)]
#[derive(Debug, Clone)]
pub struct DriverInitParamsFFI {
    pub common: CommonInitParamsFFI,
    pub kvm: KVMInitParamsFFI,
    pub memflow: MemflowInitParamsFFI,
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
        // build memflow params
        let MemflowInitParamsFFI {
            connector_name,
            connector_args,
        } = value.memflow;
        let memflow = if connector_name.is_null() {
            None
        } else {
            // parse ffi connector args
            let args = match connector_args {
                MemflowConnectorParamsFFI::Default {
                    args_arr,
                    args_arr_len,
                } => unsafe {
                    // build array slice
                    let args = std::slice::from_raw_parts(args_arr, args_arr_len);
                    // convert each C string to Rust owned String
                    let mut args_vec = Vec::new();
                    for s in args.iter() {
                        args_vec.push(CStr::from_ptr(*s).to_owned().into_string()?);
                    }
                    args_vec
                },
            };
            Some(MemflowInitParams {
                connector_name: unsafe { CStr::from_ptr(connector_name).to_owned().into_string()? },
                connector_args: Some(MemflowConnectorParams::Default { args }),
            })
        };
        Ok(DriverInitParams {
            common,
            kvm,
            memflow,
            ..Default::default()
        })
    }
}
