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
    fn read_physical(&self, paddr: u64, count: u32) -> Result<Vec<u8>,&str> {
        println!("dummy read physical - @{}, count: {}", paddr, count);
        Ok(Vec::new())
    }

    fn pause(&self) {
        println!("dummy pause");
    }

    fn resume(&self) {
        println!("dummy resume");
    }
}
