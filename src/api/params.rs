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
// - [`qemu`](https://github.com/memflow/memflow-qemu)
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

/// VirtualBox initialization parameters: selects one of the two access methods.
///
/// MachineDebugger names the VM by `common.vm_name`; VBoxVMI names it by its socket, or by
/// `common.vm_name` when `socket_path` is omitted. Without this parameter, the VirtualBox driver
/// uses the VBoxVMI method at the VM's default socket, and falls back to the MachineDebugger
/// method only if that socket file does not exist.
#[derive(Debug, Clone, PartialEq)]
pub enum VBoxInitParams {
    /// MachineDebugger access method (`virtualbox-api` or `virtualbox` feature):
    /// VirtualBox's `IMachineDebugger` API
    MachineDebugger,
    /// VBoxVMI access method (`virtualbox-vmi` or `virtualbox` feature): the VM's VBoxVMI socket.
    /// `None`: the VM's default socket, `<VirtualBox user home>/vmi/<VM UUID>.sock` or the VM's
    /// `VBoxInternal/VMI/SocketPath` extradata (`virtualbox` feature only: resolved through the
    /// VirtualBox API).
    VBoxVMI { socket_path: Option<String> },
}

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
/// use microvmi::api::params::{DriverInitParams, CommonInitParams, KVMInitParams, MemflowInitParams, VBoxInitParams};
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
/// // VirtualBox, MachineDebugger access method
/// // common.vm_name: mandatory
/// let init_params = DriverInitParams {
///     common: Some(CommonInitParams { vm_name: String::from("windows10")}),
///     virtualbox: Some(VBoxInitParams::MachineDebugger),
///     ..Default::default()
/// };
/// // VirtualBox, VBoxVMI access method
/// // virtualbox.socket_path: mandatory
/// let init_params = DriverInitParams {
///     virtualbox: Some(VBoxInitParams::VBoxVMI {
///         socket_path: String::from("/home/user/.config/VirtualBox/vmi/<vm uuid>.sock"),
///     }),
///     ..Default::default()
/// };
/// // Memflow
/// // memflow.connector_name: mandatory
/// // memflow.connector_args: optional
/// let init_params = DriverInitParams {
///     memflow: Some(MemflowInitParams { connector_name: String::from("qemu"),
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
