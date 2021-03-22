use std::convert::From;
use std::error::Error;

use pyo3::exceptions::PyValueError;
use pyo3::PyErr;

use microvmi::errors::MicrovmiError;

// create a NewType for MicrovmiError, since we cannot implement
// From trait on external types
#[derive(thiserror::Error, Debug)]
pub enum PyMicrovmiError {
    #[error("{source}")]
    Microvmi {
        #[from]
        source: MicrovmiError,
    },
    #[error("{source}")]
    Other {
        #[from]
        source: Box<dyn Error>,
    },
}

impl From<PyMicrovmiError> for PyErr {
    fn from(err: PyMicrovmiError) -> PyErr {
        PyErr::new::<PyValueError, String>(err.to_string())
    }
}
