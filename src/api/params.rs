//! This module describes initialization parameters for all libmicrovmi drivers
//!
//! The [`DriverInitParams`](struct.DriverInitParams.html) is used to pass additional driver initialization parameters.
//! You might want to check it's documentation for examples on how to initialize your driver.

/// Xen initialization parameters
#[derive(Debug, Clone, PartialEq)]
pub enum XenInitParams {}

/// KVM initialization parameters
#[derive(Debug, Clone, PartialEq)]
pub enum KVMInitParams {
    UnixSocket { path: String },
}

/// Memflow connector parameters
///
/// This enumeration reflects the possibilities to initialize Memflow
/// - default: will simply forward the string arguments to the connector
// TODO
// - [`qemu_procfs`](https://github.com/memflow/memflow-qemu-procfs)
// - [`kvm`](https://github.com/memflow/memflow-kvm)
// - [`pcileech`](https://github.com/memflow/memflow-pcileech)
// - [`coredump`](https://github.com/memflow/memflow-coredump)
#[derive(Debug, Clone, PartialEq)]
pub enum MemflowConnectorParams {
    // allow to pass an abritrary list of Strings as parameters
    Default { args: Vec<String> },
    // TODO
    // // optional vm_name, otherwise will search for the first QEMU process
    // QEMUProcFs {
    //     vm_name: Option<String>,
    // },
    // KVM {
    //     pid: u32,
    // },
    // // default value for device: "FPGA"
    // PCILeech {
    //     device: Option<String>,
    //     memmap: Option<String>,
    // },
    // Coredump {
    //     filepath: String,
    // },
}

/// Memflow initialization parameters
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MemflowInitParams {
    /// connector name
    pub connector_name: String,
    /// optional connector initialization parameters
    pub connector_args: Option<MemflowConnectorParams>,
}

/// VirtualBox initialization parameters
#[derive(Debug, Clone, PartialEq)]
pub enum VBoxInitParams {}

/// Common initialization parameters
///
/// These parameters are shared by two or more drivers, and are stored in this struct
/// to avoid duplication and simplify the API
#[repr(C)]
#[derive(Default, Debug, Clone, PartialEq)]
pub struct CommonInitParams {
    pub vm_name: String,
}

/// This struct is used to specify the initialization parameters for all drivers
///
/// # Examples
///
/// ```no_run
/// // Xen
/// // common.vm_name: mandatory
/// use microvmi::api::params::{DriverInitParams, CommonInitParams, KVMInitParams, MemflowInitParams};
/// let init_params = DriverInitParams {
///     common: Some(CommonInitParams { vm_name: String::from("windows10")}),
///     ..Default::default()
/// };
/// // KVM
/// // common.vm_name: mandatory
/// // kvm.unix_socket: mandatory
/// let init_params = DriverInitParams {
///     common: Some(CommonInitParams { vm_name: String::from("windows10")}),
///     kvm: Some(KVMInitParams::UnixSocket { path: String::from("/tmp/introspector")}),
///     ..Default::default()
/// };
/// // VirtualBox
/// // common.vm_name: mandatory
/// let init_params = DriverInitParams {
///     common: Some(CommonInitParams { vm_name: String::from("windows10")}),
///     ..Default::default()
/// };
/// // Memflow
/// // memflow.connector_name: mandatory
/// // memflow.connector_args: optional
/// let init_params = DriverInitParams {
///     memflow: Some(MemflowInitParams { connector_name: String::from("qemu_procfs"), ///
///         ..Default::default()}),
///     ..Default::default()
/// };
/// ```
#[derive(Default, Debug, Clone, PartialEq)]
pub struct DriverInitParams {
    pub common: Option<CommonInitParams>,
    pub xen: Option<XenInitParams>,
    pub kvm: Option<KVMInitParams>,
    pub memflow: Option<MemflowInitParams>,
    pub virtualbox: Option<VBoxInitParams>,
}
