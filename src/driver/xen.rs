extern crate xenctrl;
use crate::api;
use xenctrl::Xc;

// unit struct
pub struct Xen {
    xc: Xc,
}

impl Xen {

    pub fn new() -> Self {
        println!("Xen driver init !");
        let xen = Xen { xc: Xc::new() };
        xen
    }


}

impl api::Introspectable for Xen {
    fn close(&self) {
        println!("Xen driver close !");
        // self.xc::close();
    }
}

