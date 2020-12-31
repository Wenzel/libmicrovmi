use crate::api::{DriverError, DriverInitParam, Introspectable};

// unit struct
pub struct Dummy;

impl Dummy {
    pub fn new(domain_name: &str, _init_option: Option<DriverInitParam>) -> Self {
        debug!("init on {}", domain_name);
        Dummy
    }
}

impl Introspectable for Dummy {
    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn DriverError>> {
        debug!("read physical - @{}, {:#?}", paddr, buf);
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn DriverError>> {
        debug!("get max physical address");
        Ok(0)
    }

    fn pause(&mut self) -> Result<(), Box<dyn DriverError>> {
        debug!("pause");
        Ok(())
    }

    fn resume(&mut self) -> Result<(), Box<dyn DriverError>> {
        debug!("resume");
        Ok(())
    }
}
