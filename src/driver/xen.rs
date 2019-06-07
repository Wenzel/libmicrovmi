extern crate xenctrl;
use crate::api;
use xenctrl::Xc;

// unit struct
pub struct Xen {
    xc: Xc,
    domid: u32,
}

impl Xen {

    pub fn new(domid: u32) -> Self {
        println!("Xen driver init !");
        let xc = Xc::new();
        let xen = Xen {
            xc: xc,
            domid: domid
        };
        xen
    }
}

impl api::Introspectable for Xen {
    fn close(&mut self) {
        println!("Xen driver close !");
        self.xc.close();
    }
}

