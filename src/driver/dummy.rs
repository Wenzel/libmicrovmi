use crate::api;

// unit struct
pub struct Dummy;

impl api::Introspectable for Dummy {
    fn new(&self) {
        println!("dummy driver init !");
    }

    fn close(&self) {
        println!("dummy driver close !");
    }
}
