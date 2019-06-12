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
    fn pause(&self) {
        println!("dummy pause");
    }

    fn resume(&self) {
        println!("dummy resume");
    }
}
