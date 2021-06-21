/// This module describes initialization parameters for all libmicrovmi drivers

/// Xen initialization parameters
#[derive(Debug, Clone, PartialEq)]
pub enum XenInitParams {}

/// KVM initialization parameters
#[derive(Debug, Clone, PartialEq)]
pub enum KVMInitParams {
    UnixSocket { path: String },
}

/// VirtualBox initialization parameters
#[derive(Debug, Clone, PartialEq)]
pub enum VBoxInitParams {}

/// Common initialization parameters
///
/// These parameters are shared by two or more drivers, and are stored in this struct
/// to avoid duplication and simplify the API
#[derive(Debug, Clone, PartialEq)]
pub struct CommonInitParams {
    pub vm_name: String,
}

/// This struct is used to specify the initialization parameters for all drivers
#[derive(Default, Debug, Clone, PartialEq)]
pub struct DriverInitParams {
    pub common: Option<CommonInitParams>,
    pub xen: Option<XenInitParams>,
    pub kvm: Option<KVMInitParams>,
    pub virtualbox: Option<VBoxInitParams>,
}
