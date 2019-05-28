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

    fn close(&self) {
        println!("dummy driver close !");
    }
}
