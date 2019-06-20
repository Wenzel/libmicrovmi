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

    fn pause(&self) {
        println!("dummy pause");
    }

    fn resume(&self) {
        println!("dummy resume");
    }
}
