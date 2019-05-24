use crate::intro;

// unit struct
pub struct Dummy;

impl intro::Introspectable for Dummy {
    fn new(&self) {
        println!("dummy driver init !");
    }

    fn close(&self) {
        println!("dummy driver close !");
    }
}
