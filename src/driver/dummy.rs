use crate::api::{DriverError, DriverInitParam, Introspectable};

#[derive(thiserror::Error, Debug)]
pub enum DummyDriverError {
    #[error("dummy error")]
    DummyError,
}

// unit struct
pub struct Dummy;

impl Dummy {
    pub fn new(domain_name: &str, _init_option: Option<DriverInitParam>) -> Self {
        debug!("init on {}", domain_name);
        Dummy
    }
}

impl Introspectable for Dummy {
    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), DriverError> {
        debug!("read physical - @{}, {:#?}", paddr, buf);
        if paddr == 0 {
            return Err(DummyDriverError::DummyError.into());
        }
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, DriverError> {
        debug!("get max physical address");
        Ok(0)
    }

    fn pause(&mut self) -> Result<(), DriverError> {
        debug!("pause");
        Ok(())
    }

    fn resume(&mut self) -> Result<(), DriverError> {
        debug!("resume");
        Ok(())
    }
}
