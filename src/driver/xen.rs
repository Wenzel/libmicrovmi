extern crate xenctrl;
use crate::api;
use xenctrl::Xc;

// unit struct
pub struct Xen {
    xc: Xc,
}

impl api::Introspectable for Xen {
    fn new(&self) {
        println!("Xen driver init !");
        self.xc = Xc::new();
    }

    fn close(&self) {
        println!("Xen driver close !");
        self.xc::close();
    }
}

