use crate::driver::dummy::DummyDriverError;
use crate::api::Introspectable;

#[non_exhaustive]
pub enum AnyDriver {
    None,
    Dummy(Box<dyn Introspectable<DriverError = DummyDriverError>>),
}

#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum DriverError {
    #[cfg(feature = "xen")]
    Xen(XenDriverError),
    #[error(transparent)]
    Dummy(#[from] DummyDriverError),
}

impl Introspectable for AnyDriver {
    type DriverError = DriverError;

    fn get_vcpu_count(&self) -> Result<u16, Self::DriverError> {
        match self {
            Self::None => panic!("Oh shit!"),
            Self::Dummy(d) => Ok(d.get_vcpu_count()?),
        }
    }
}
