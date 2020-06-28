use std::error::Error;

use fdp::FDP;

use crate::api::Introspectable;

// unit struct
#[derive(Debug)]
pub struct VBox {
    fdp: FDP,
}

impl VBox {
    pub fn new(domain_name: &str) -> Self {
        // init FDP
        let fdp = FDP::new(domain_name);
        VBox { fdp }
    }
}

impl Introspectable for VBox {
    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        self.fdp.read_physical_memory(paddr, buf)
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        self.fdp.get_physical_memory_size()
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        self.fdp.pause()
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        self.fdp.resume()
    }
}
