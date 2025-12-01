use libloading::Error as libError;
use std::{
    error,
    fmt::{self, Debug, Display, Formatter},
};

pub struct XcError {
    desc: String,
}

impl XcError {
    pub fn new(s: &str) -> Self {
        Self { desc: s.to_owned() }
    }
}

impl Display for XcError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.desc)
    }
}

impl Debug for XcError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.desc)
    }
}

impl error::Error for XcError {}

impl From<libError> for XcError {
    fn from(e: libError) -> Self {
        XcError {
            desc: e.to_string(),
        }
    }
}
