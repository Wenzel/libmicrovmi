use crate::api;

// unit struct
pub struct Dummy;

impl Dummy {

    pub fn new() -> Self {
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

    fn close(&mut self) {
        println!("dummy driver close");
    }
}
