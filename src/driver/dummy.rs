use crate::api;

// unit struct
pub struct Dummy;

impl Dummy {

    pub fn new(domain_name: &String) -> Self {
        println!("dummy driver init on {}", domain_name);
        let drv = Dummy { };
        return drv;
    }
}

impl api::Introspectable for Dummy {
    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),&str> {
        println!("dummy read physical - @{}, {:#?}", paddr, buf);
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64,&str> {
        println!("dummy get max physical address");
        Ok(0)
    }

    fn pause(&self) {
        println!("dummy pause");
    }

    fn resume(&self) {
        println!("dummy resume");
    }
}
