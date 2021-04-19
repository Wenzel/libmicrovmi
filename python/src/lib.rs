mod errors;

use log::{debug, info};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use errors::PyMicrovmiError;
use microvmi::api as rapi; // rust api
use microvmi::Microvmi;
use pyo3::types::{PyByteArray, PyBytes};
use std::io::{Read, Seek, SeekFrom};

/// microvmi Python module declaration
#[pymodule]
fn pymicrovmi(_py: Python, m: &PyModule) -> PyResult<()> {
    // init the env logger at module init
    env_logger::init();

    m.add_class::<MicrovmiExt>()?;
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
    const KVM: u32 = 0;
    #[classattr]
    const VIRTUALBOX: u32 = 1;
    #[classattr]
    const XEN: u32 = 2;
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
    #[pyo3(get)]
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
struct MicrovmiExt {
    microvmi: Microvmi,
}

#[pymethods]
impl MicrovmiExt {
    // enums are not available in PyO3 (yet)
    // TODO: docstring is not exposed in Python (bug ?)
    /// initializes libmicrovmi from the specified domain name
    /// if driver_type is None, every driver compiled in libmicrovmi will be tested,
    /// and the first one that succeeds will be returned, or an error
    ///
    /// Args:
    ///     domain_name (str): the domain name
    ///     driver_type (int, optional): the hypervisor driver type on which the library should be initialized.
    ///     init_param (DriverInitParam, optional): additional initialization parameters for driver initialization
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
        let rust_driver_type = driver_type
            .map(|drv_type| match drv_type {
                DriverType::KVM => Ok(rapi::DriverType::KVM),
                DriverType::VIRTUALBOX => Ok(rapi::DriverType::VirtualBox),
                DriverType::XEN => Ok(rapi::DriverType::Xen),
                _ => Err(PyValueError::new_err(format!(
                    "Invalid value for DriverType: {}",
                    drv_type
                ))),
            })
            .transpose()?;
        // convert Python DriverInitParam to rust API DriverinitParam
        let rust_init_param: Option<rapi::DriverInitParam> =
            init_param.map(|param| match param.param_type {
                DriverInitParamType::KVMiUnixSocket => {
                    rapi::DriverInitParam::KVMiSocket(param.param_data_string)
                }
            });
        let microvmi = Microvmi::new(domain_name, rust_driver_type, rust_init_param)
            .map_err(PyMicrovmiError::from)?;
        Ok(MicrovmiExt { microvmi })
    }

    /// read VM physical memory starting from paddr, of a given size
    ///
    /// Args:
    ///     paddr: (int) physical address from where the read operation should start
    ///     size: (int) size of the read operation
    ///
    /// Returns:
    ///     Tuple[bytes, int]: the read operation result and the amount bytes read
    fn read_physical<'p>(
        &mut self,
        py: Python<'p>,
        paddr: u64,
        size: usize,
    ) -> PyResult<(&'p PyBytes, u64)> {
        let mut bytes_read: u64 = 0;
        self.microvmi.memory.seek(SeekFrom::Start(paddr))?;
        let pybuffer: &PyBytes = PyBytes::new_with(py, size, |mut buffer| {
            bytes_read = self.microvmi.memory.read(&mut buffer).unwrap_or(0) as u64;
            Ok(())
        })?;

        Ok((pybuffer, bytes_read))
    }

    /// read VM physical memory starting from paddr into the given buffer
    ///
    /// Args:
    ///     paddr (int): the physical address to start reading from
    ///     buffer (bytearray): the buffer to read into
    fn read_physical_into(&mut self, paddr: u64, buffer: &PyByteArray) -> PyResult<u64> {
        let mut_buf: &mut [u8] = unsafe { buffer.as_bytes_mut() };
        // ignore read error
        self.microvmi.memory.seek(SeekFrom::Start(paddr))?;
        let bytes_read = self.microvmi.memory.read(mut_buf).unwrap_or(0) as u64;
        Ok(bytes_read)
    }

    /// pause the VM
    fn pause(&mut self) -> PyResult<()> {
        Ok(self.microvmi.pause().map_err(PyMicrovmiError::from)?)
    }

    /// resume the VM
    fn resume(&mut self) -> PyResult<()> {
        Ok(self.microvmi.resume().map_err(PyMicrovmiError::from)?)
    }

    /// get maximum physical address
    ///
    /// Returns:
    ///     int: the maximum physical address
    fn get_max_physical_addr(&self) -> PyResult<u64> {
        let max_addr = self
            .microvmi
            .get_max_physical_addr()
            .map_err(PyMicrovmiError::from)?;
        Ok(max_addr)
    }
}
