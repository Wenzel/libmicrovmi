use std::error::Error;
use crate::api;
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, TH32CS_SNAPPROCESS};
use winapi::um::winnt::HANDLE;

// unit struct
#[derive(Debug)]
pub struct HyperV {
	a: i32
}

impl HyperV {

    pub fn new(domain_name: &str) -> Self {
        println!("HyperV driver init on {}", domain_name);
        // snapshot
        let snapshot_handle: HANDLE = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
        };

        let hyperv = HyperV {
			a: 0
        };
        hyperv
    }

    fn close(&mut self) {
        println!("HyperV driver close");
    }
}

impl api::Introspectable for HyperV {

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),Box<dyn Error>> {
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64,Box<dyn Error>> {
        Ok(0)
    }

    fn pause(&mut self) -> Result<(),Box<dyn Error>> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(),Box<dyn Error>> {
        Ok(())
    }

}