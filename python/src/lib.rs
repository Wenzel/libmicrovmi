use log::{debug, info};
use microvmi::api as rapi; // rust api
use microvmi::init;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// microvmi Python module declaration
#[pymodule]
fn pymicrovmi(_py: Python, m: &PyModule) -> PyResult<()> {
    // init the env logger at module init
    env_logger::init();

    m.add_class::<Microvmi>()?;
    m.add_class::<DriverType>()?;
    m.add_class::<DriverInitParam>()?;

    Ok(())
}

// fake enum impl for driver type
#[pyclass]
struct DriverType {}

#[pymethods]
impl DriverType {
    #[classattr]
    const HYPERV: u32 = 0;
    #[classattr]
    const KVM: u32 = 1;
    #[classattr]
    const VIRTUALBOX: u32 = 2;
    #[classattr]
    const XEN: u32 = 3;
}

// this will not be exported to Python
#[derive(Debug, Copy, Clone)]
enum DriverInitParamType {
    KVMiUnixSocket = 0,
}

// exposing DriverInitParam to Python is bit more complicated
// as it's an enum with associated data
// the proposed implementation is exposing static method returning an
// initialized instance of DriverInitParam struct
/// Manages additional driver initialization parameters
#[pyclass]
#[derive(Debug, Clone)]
struct DriverInitParam {
    pub param_type: DriverInitParamType,
    pub param_data_string: String,
}

#[pymethods]
impl DriverInitParam {
    /// initialize a DriverInitParam for the KVM driver, with a Unix socket
    #[staticmethod]
    fn kvmi_unix_socket(socket: &str) -> Self {
        DriverInitParam {
            param_type: DriverInitParamType::KVMiUnixSocket,
            param_data_string: socket.to_string(),
        }
    }
}

/// Main class to interact with libmicrovmi
// A class marked as unsendable will panic when accessed by another thread.
// TODO: make Introspectable trait inherit Send trait, and make the drivers implementation
// compatible
#[pyclass(unsendable)]
struct Microvmi {
    _driver: Box<dyn rapi::Introspectable>,
}

#[pymethods]
impl Microvmi {
    // TODO: pass driver type and driver parameters
    // enums are not available in PyO3 (yet)
    /// initializes libmicrovmi from the specified domain name
    #[new]
    #[args(domain_name, driver_type = "None", init_param = "None")]
    fn new(
        domain_name: &str,
        driver_type: Option<u32>,
        init_param: Option<DriverInitParam>,
    ) -> PyResult<Self> {
        info!("Microvmi Python init");
        // convert Python DriverType to rust API DriverType
        debug!(
            "Microvmi Python init driver_type: {:?}, init_param: {:?}",
            driver_type, init_param
        );
        let rust_driver_type: Option<rapi::DriverType> = if let Some(drv_type) = driver_type {
            Some(match drv_type {
                DriverType::HYPERV => Ok(rapi::DriverType::HyperV),
                DriverType::KVM => Ok(rapi::DriverType::KVM),
                DriverType::VIRTUALBOX => Ok(rapi::DriverType::VirtualBox),
                DriverType::XEN => Ok(rapi::DriverType::Xen),
                _ => Err(PyValueError::new_err(format!(
                    "Invalid value for DriverType: {}",
                    drv_type
                ))),
            }?)
        } else {
            None
        };
        // convert Python DriverInitParam to rust API DriverinitParam
        let rust_init_param: Option<rapi::DriverInitParam> = if let Some(param) = init_param {
            Some(
                #[allow(unreachable_patterns)]
                match param.param_type {
                    DriverInitParamType::KVMiUnixSocket => {
                        Ok(rapi::DriverInitParam::KVMiSocket(param.param_data_string))
                    }
                    _ => Err(PyValueError::new_err(format!(
                        "Invalid value for DriverInitParam type: {:?}",
                        param.param_type
                    ))),
                }?,
            )
        } else {
            None
        };
        let _driver = init(domain_name, rust_driver_type, rust_init_param);
        Ok(Microvmi { _driver })
    }
}
