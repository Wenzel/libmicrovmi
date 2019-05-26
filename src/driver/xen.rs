use crate::api;

// unit struct
pub struct Xen;

impl api::Introspectable for Xen {
    fn new(&self) {
        println!("Xen driver init !");
    }

    fn close(&self) {
        println!("Xen driver close !");
    }
}

