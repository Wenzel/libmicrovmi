/// This modules defines the driver initialization parameters to be exposed from Python
use pyo3::prelude::*;

/// equivalent of `CommonInitParams` for Python
#[pyclass(from_py_object)]
#[derive(Default, Debug, Clone)]
pub struct CommonInitParamsPy {
    #[pyo3(get, set)]
    pub vm_name: String,
}

#[pymethods]
impl CommonInitParamsPy {
    #[new]
    fn new() -> Self {
        Self::default()
    }
}

/// equivalent of `KVMInitParams` for Python
#[pyclass(from_py_object)]
#[derive(Default, Debug, Clone)]
pub struct KVMInitParamsPy {
    #[pyo3(get, set)]
    pub unix_socket: String,
}

#[pymethods]
impl KVMInitParamsPy {
    #[new]
    fn new() -> Self {
        Self::default()
    }
}

/// equivalent of `MemflowInitParams` for Python
#[pyclass(from_py_object)]
#[derive(Default, Debug, Clone)]
pub struct MemflowInitParamsPy {
    #[pyo3(get, set)]
    pub connector_name: String,
    #[pyo3(get, set)]
    pub connector_args: Vec<String>,
}

#[pymethods]
impl MemflowInitParamsPy {
    #[new]
    fn new(name: &str) -> Self {
        Self {
            connector_name: String::from(name),
            connector_args: Vec::new(),
        }
    }
}

/// equivalent of `VBoxInitParams` for Python: selects the VirtualBox access method
///
/// Usage from Python: `VBoxInitParamsPy.MachineDebugger()`,
/// `VBoxInitParamsPy.VBoxVMI(socket_path="/path/to/vm.sock")`, or `VBoxInitParamsPy.VBoxVMI()`
/// for the VM's default socket. Leave `DriverInitParamsPy.virtualbox` unset for the default
/// access method.
#[pyclass]
#[derive(Debug, Clone)]
pub enum VBoxInitParamsPy {
    // struct variant (not unit) so variants with fields can be added without changing the Python API
    MachineDebugger {},
    #[pyo3(constructor = (socket_path = None))]
    VBoxVMI {
        socket_path: Option<String>,
    },
}

/// equivalent of `DriverInitParams` for Python
///
/// # Examples
///
/// Usage from Python
/// ```Python
/// from microvmi import DriverInitParamsPy, CommonInitParamsPy, KVMInitParamsPy
/// # setup common params
/// common = CommonInitParamsPy()
/// common.vm_name = "windows10"
/// # setup kvm params
/// kvm = KVMInitParamsPy()
/// kvm.unix_socket = "/tmp/introspector"
/// # finalize
/// init_params = DriverInitParamsPy()
/// init_params.common = common
/// init_params.kvm = kvm
/// # or select a VirtualBox access method
/// init_params.virtualbox = VBoxInitParamsPy.MachineDebugger()
/// init_params.virtualbox = VBoxInitParamsPy.VBoxVMI(socket_path="/path/to/vm.sock")
/// ```
#[pyclass(from_py_object)]
#[derive(Default, Debug, Clone)]
pub struct DriverInitParamsPy {
    #[pyo3(get, set)]
    pub common: Option<CommonInitParamsPy>,
    #[pyo3(get, set)]
    pub kvm: Option<KVMInitParamsPy>,
    #[pyo3(get, set)]
    pub memflow: Option<MemflowInitParamsPy>,
    #[pyo3(get, set)]
    pub virtualbox: Option<VBoxInitParamsPy>,
}

#[pymethods]
impl DriverInitParamsPy {
    #[new]
    fn new() -> Self {
        Self::default()
    }
}
