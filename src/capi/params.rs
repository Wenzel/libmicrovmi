use crate::api::params::{
    CommonInitParams, DriverInitParams, KVMInitParams, MemflowConnectorParams, MemflowInitParams,
    VBoxInitParams,
};
use std::convert::TryFrom;
use std::error::Error;
use std::ffi::CStr;
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

/// `VBoxInitParamsFFI.access_method`: no `VBoxInitParams` (the VirtualBox driver's default access method)
pub const VBOX_ACCESS_METHOD_NONE: u32 = 0;
/// `VBoxInitParamsFFI.access_method`: `VBoxInitParams::MachineDebugger`
pub const VBOX_ACCESS_METHOD_MACHINE_DEBUGGER: u32 = 1;
/// `VBoxInitParamsFFI.access_method`: `VBoxInitParams::VBoxVMI`, with `VBoxInitParamsFFI.socket_path`
pub const VBOX_ACCESS_METHOD_VBOXVMI: u32 = 2;

/// equivalent of `Option<VBoxInitParams>` with C compatibility
///
/// A zero-initialized struct means no `VBoxInitParams`.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VBoxInitParamsFFI {
    /// one of the `VBOX_ACCESS_METHOD_*` constants
    // plain integer, not a Rust enum: an out-of-range value from C would be UB in an enum
    pub access_method: u32,
    /// `VBOX_ACCESS_METHOD_VBOXVMI`: the VM's VBoxVMI socket path (mandatory); ignored otherwise
    pub socket_path: *mut c_char,
}

/// equivalent of `DriverInitParam` with C compatibility
#[repr(C)]
#[derive(Debug, Clone)]
pub struct DriverInitParamsFFI {
    pub common: CommonInitParamsFFI,
    pub kvm: KVMInitParamsFFI,
    pub memflow: MemflowInitParamsFFI,
    pub virtualbox: VBoxInitParamsFFI,
}

// convert from FFI type to Rust type
impl TryFrom<DriverInitParamsFFI> for DriverInitParams {
    type Error = Box<dyn Error>;

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
                } => {
                    // Only call from_raw_parts if args_arr is non-null and args_arr_len > 0
                    if !args_arr.is_null() && args_arr_len > 0 {
                        // build array slice
                        let args = unsafe { std::slice::from_raw_parts(args_arr, args_arr_len) };
                        // convert each C string to Rust owned String
                        let mut args_vec = Vec::with_capacity(args.len());
                        for s in args.iter() {
                            args_vec.push(unsafe { CStr::from_ptr(*s) }.to_owned().into_string()?);
                        }
                        Some(MemflowConnectorParams::Default { args: args_vec })
                    } else {
                        // no args provided
                        Some(MemflowConnectorParams::Default { args: vec![] })
                    }
                }
            };
            Some(MemflowInitParams {
                connector_name: unsafe { CStr::from_ptr(connector_name) }
                    .to_owned()
                    .into_string()?,
                connector_args: args,
            })
        };
        let virtualbox = match value.virtualbox.access_method {
            VBOX_ACCESS_METHOD_NONE => None,
            VBOX_ACCESS_METHOD_MACHINE_DEBUGGER => Some(VBoxInitParams::MachineDebugger),
            VBOX_ACCESS_METHOD_VBOXVMI => {
                let path = value.virtualbox.socket_path;
                if path.is_null() {
                    return Err("VBOX_ACCESS_METHOD_VBOXVMI requires a socket_path".into());
                }
                Some(VBoxInitParams::VBoxVMI {
                    socket_path: unsafe { CStr::from_ptr(path) }.to_owned().into_string()?,
                })
            }
            other => return Err(format!("unknown VirtualBox access method: {}", other).into()),
        };
        Ok(DriverInitParams {
            common,
            kvm,
            memflow,
            virtualbox,
            ..Default::default()
        })
    }
}
