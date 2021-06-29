/// This module describes initialization parameters for all libmicrovmi drivers

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
/// - [`qemu_procfs`](https://github.com/memflow/memflow-qemu-procfs)
/// - [`kvm`](https://github.com/memflow/memflow-kvm)
/// - [`pcileech`](https://github.com/memflow/memflow-pcileech)
/// - [`coredump`](https://github.com/memflow/memflow-coredump)
/// - unknown: will simply forward the string arguments to the connector
#[derive(Debug, Clone, PartialEq)]
pub enum MemflowConnectorParams {
    // optional vm_name, otherwise will search for the first QEMU process
    QEMUProcFs {
        vm_name: Option<String>,
    },
    KVM {
        pid: u32,
    },
    // default value for device: "FPGA"
    PCILeech {
        device: Option<String>,
        memmap: Option<String>,
    },
    Coredump {
        filepath: String,
    },
    // allow to pass an abritrary list of Strings as parameters
    Unknown {
        args: Vec<String>,
    },
}

/// Memflow initialization parameters
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MemflowInitParams {
    /// optional connector name
    pub connector_name: Option<String>,
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
#[derive(Default, Debug, Clone, PartialEq)]
pub struct DriverInitParams {
    pub common: Option<CommonInitParams>,
    pub xen: Option<XenInitParams>,
    pub kvm: Option<KVMInitParams>,
    pub memflow: Option<MemflowInitParams>,
    pub virtualbox: Option<VBoxInitParams>,
}
