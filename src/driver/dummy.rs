use crate::api::{DriverType, Introspectable};
use std::error::Error;

// unit struct
pub struct Dummy;

impl Dummy {
    pub fn new(domain_name: &str) -> Self {
        debug!("init on {}", domain_name);
        Dummy
    }
}

impl Introspectable for Dummy {
    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        debug!("read physical - @{}, {:#?}", paddr, buf);
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        debug!("get max physical address");
        Ok(0)
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("pause");
        Ok(())
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("resume");
        Ok(())
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::Dummy
    }
}
