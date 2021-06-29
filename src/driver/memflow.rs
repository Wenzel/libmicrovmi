use crate::api::params::DriverInitParams;
use crate::api::{DriverType, Introspectable};
use std::error::Error;

pub struct Memflow;

impl Memflow {
    pub fn new(_init_params: DriverInitParams) -> Result<Self, Box<dyn Error>> {
        Ok(Memflow {})
    }
}

impl Introspectable for Memflow {
    fn get_driver_type(&self) -> DriverType {
        DriverType::Memflow
    }
}
