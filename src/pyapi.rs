use crate::init;

use crate::api::Introspectable;
use pyo3::prelude::*;

/// microvmi Python module declaration
#[pymodule]
fn microvmi(_py: Python, m: &PyModule) -> PyResult<()> {
    // init the env logger at module init
    env_logger::init();

    m.add_class::<Microvmi>()?;

    Ok(())
}

/// Main class to interact with libmicrovmi
// A class marked as unsendable will panic when accessed by another thread.
// TODO: make Introspectable trait inherit Send trait, and make the drivers implementation
// compatible
#[pyclass(unsendable)]
struct Microvmi {
    _driver: Box<dyn Introspectable>,
}

#[pymethods]
impl Microvmi {
    // TODO: pass driver type and driver parameters
    // enums are not available in PyO3 (yet)
    /// initializes libmicrovmi from the specified domain name
    #[new]
    fn new(domain_name: &str) -> Self {
        let _driver = init(domain_name, None, None);
        Microvmi { _driver }
    }
}
