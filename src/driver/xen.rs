extern crate xenctrl;
use crate::api;
use xenctrl::Xc;

// unit struct
pub struct Xen {
    xc: Xc,
    dom_name: String,
    domid: u32,
}

impl Xen {

    pub fn new(domain_name: &String) -> Self {
        println!("Xen driver init on {}", domain_name);
        let xc = Xc::new().unwrap();
        let xen = Xen {
            xc: xc,
            dom_name: domain_name.clone(),
            domid: 0,
        };
        xen
    }

    fn close(&mut self) {
        println!("Xen driver close");
        self.xc.close().unwrap();
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

}

impl Drop for Xen {
    fn drop(&mut self) {
        self.close();
    }
}
