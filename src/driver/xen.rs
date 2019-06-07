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
        println!("Xen driver init");
        let xc = Xc::new();
        let xen = Xen {
            xc: xc,
            domid: domid
        };
        xen
    }
}

impl api::Introspectable for Xen {

    fn pause(&self) {
        println!("Xen driver pause");
        self.xc.domain_pause(self.domid).unwrap();
    }

    fn resume(&self) {
        println!("Xen driver resume");
        self.xc.domain_unpause(self.domid).unwrap();
    }

    fn close(&mut self) {
        println!("Xen driver close");
        self.xc.close();
    }
}

