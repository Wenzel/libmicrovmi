mod errors;
mod params;

use log::{debug, info};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes};

use errors::PyMicrovmiError;
use microvmi::api as rapi; // rust api
use microvmi::api::params as rparams; // rust params
use microvmi::init;
use params::{CommonInitParamsPy, DriverInitParamsPy, KVMInitParamsPy, MemflowInitParamsPy};

/// microvmi Python module declaration
#[pymodule]
fn pymicrovmi(_py: Python, m: &PyModule) -> PyResult<()> {
    // init the env logger at module init
    env_logger::init();

    m.add_class::<MicrovmiExt>()?;
    m.add_class::<DriverType>()?;
    m.add_class::<DriverInitParamsPy>()?;
    m.add_class::<CommonInitParamsPy>()?;
    m.add_class::<KVMInitParamsPy>()?;
    m.add_class::<MemflowInitParamsPy>()?;

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

/// Main class to interact with libmicrovmi
// A class marked as unsendable will panic when accessed by another thread.
// TODO: make Introspectable trait inherit Send trait, and make the drivers implementation
// compatible
#[pyclass(unsendable)]
struct MicrovmiExt {
    driver: Box<dyn rapi::Introspectable>,
}

#[pymethods]
impl MicrovmiExt {
    // enums are not available in PyO3 (yet)
    // TODO: docstring is not exposed in Python (bug ?)
    /// initializes libmicrovmi
    /// if driver_type is None, every driver compiled in libmicrovmi will be tested,
    /// and the first one that succeeds will be returned, or an error
    ///
    /// Args:
    ///     driver_type (int, optional): the hypervisor driver type on which the library should be initialized.
    ///     init_param (DriverInitParamPy, optional): initialization parameters for driver initialization
    #[new]
    #[args(driver_type = "None", init_params = "None")]
    fn new(driver_type: Option<u32>, init_params: Option<DriverInitParamsPy>) -> PyResult<Self> {
        info!("Microvmi Python init");
        debug!(
            "Microvmi Python init driver_type: {:?}, init_param: {:?}",
            driver_type, init_params
        );
        // convert Python DriverType to rust API DriverType
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
        let rust_init_params = init_params.map(|v| rparams::DriverInitParams {
            common: v
                .common
                .map(|k| rparams::CommonInitParams { vm_name: k.vm_name }),
            kvm: v.kvm.map(|k| rparams::KVMInitParams::UnixSocket {
                path: k.unix_socket,
            }),
            memflow: v.memflow.map(|k| rparams::MemflowInitParams {
                connector_name: k.connector_name,
                connector_args: Some(rparams::MemflowConnectorParams::Default {
                    args: k.connector_args,
                }),
            }),
            ..Default::default()
        });

        let driver = init(rust_driver_type, rust_init_params).map_err(PyMicrovmiError::from)?;
        Ok(MicrovmiExt { driver })
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
        &self,
        py: Python<'p>,
        paddr: u64,
        size: usize,
    ) -> PyResult<(&'p PyBytes, u64)> {
        let mut bytes_read: u64 = 0;
        let pybuffer: &PyBytes = PyBytes::new_with(py, size, |buffer| {
            self.driver
                .read_physical(paddr, buffer, &mut bytes_read)
                .ok();
            Ok(())
        })?;

        Ok((pybuffer, bytes_read))
    }

    /// read VM physical memory starting from paddr into the given buffer
    ///
    /// Args:
    ///     paddr (int): the physical address to start reading from
    ///     buffer (bytearray): the buffer to read into
    fn read_physical_into(&self, paddr: u64, buffer: &PyByteArray) -> u64 {
        let mut_buf: &mut [u8] = unsafe { buffer.as_bytes_mut() };
        let mut bytes_read: u64 = 0;
        // ignore read error
        self.driver
            .read_physical(paddr, mut_buf, &mut bytes_read)
            .ok();
        bytes_read
    }

    /// pause the VM
    fn pause(&mut self) -> PyResult<()> {
        Ok(self.driver.pause().map_err(PyMicrovmiError::from)?)
    }

    /// resume the VM
    fn resume(&mut self) -> PyResult<()> {
        Ok(self.driver.resume().map_err(PyMicrovmiError::from)?)
    }

    /// get maximum physical address
    ///
    /// Returns:
    ///     int: the maximum physical address
    fn get_max_physical_addr(&self) -> PyResult<u64> {
        let max_addr = self
            .driver
            .get_max_physical_addr()
            .map_err(PyMicrovmiError::from)?;
        Ok(max_addr)
    }
}
