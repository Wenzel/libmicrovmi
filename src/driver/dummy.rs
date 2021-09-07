use crate::api::params::DriverInitParams;
use std::error::Error;

#[derive(Debug)]
pub struct Dummy;

use crate::api::events::{Event, EventReplyType, InterceptType};
use crate::api::registers::Registers;
use crate::api::{Access, DriverType, Introspectable, PAGE_SIZE};

impl Dummy {
    pub fn new(_init_parms: DriverInitParams) -> Result<Self, Box<dyn Error>> {
        Ok(Dummy {})
    }
}

impl Introspectable for Dummy {
    fn read_physical(
        &self,
        _paddr: u64,
        _buf: &mut [u8],
        _bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        Ok(1024 * 1024 * 10)
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::Dummy
    }
}
