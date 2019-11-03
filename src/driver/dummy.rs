use std::error::Error;
use crate::api;

// unit struct
pub struct Dummy;

impl Dummy {

    pub fn new(domain_name: &str) -> Self {
        debug!("dummy driver init on {}", domain_name);
        Dummy
    }
}

impl api::Introspectable for Dummy {
    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),Box<dyn Error>> {
        debug!("dummy read physical - @{}, {:#?}", paddr, buf);
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64,Box<dyn Error>> {
        debug!("dummy get max physical address");
        Ok(0)
    }

    fn pause(&mut self) -> Result<(),Box<dyn Error>> {
        debug!("dummy pause");
        Ok(())
    }

    fn resume(&mut self) -> Result<(),Box<dyn Error>> {
        debug!("dummy resume");
        Ok(())
    }
}
