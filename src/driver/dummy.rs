use crate::api::{DriverInitParam, Introspectable};

// unit struct
pub struct Dummy;

impl Dummy {
    pub fn new(domain_name: &str, _init_option: Option<DriverInitParam>) -> Self {
        debug!("init on {}", domain_name);
        Dummy
    }
}

impl Introspectable for Dummy {
    type DriverError = DummyDriverError;

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Self::DriverError> {
        debug!("read physical - @{}, {:#?}", paddr, buf);
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Self::DriverError> {
        debug!("get max physical address");
        Ok(0)
    }

    fn pause(&mut self) -> Result<(), Self::DriverError> {
        debug!("pause");
        Ok(())
    }

    fn resume(&mut self) -> Result<(), Self::DriverError> {
        debug!("resume");
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DummyDriverError {
    #[error("test")]
    Test,
}
