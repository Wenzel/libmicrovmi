extern crate xenctrl;
extern crate xenstore;
use crate::api;
use xenctrl::Xc;
use xenstore::{Xs, XBTransaction, XsOpenFlags};

// unit struct
#[derive(Debug)]
pub struct Xen {
    xc: Xc,
    dom_name: String,
    domid: u32,
}

impl Xen {

    pub fn new(domain_name: &String) -> Self {
        println!("Xen driver init on {}", domain_name);
        // find domain name in xenstore
        let xs = Xs::new(XsOpenFlags::ReadOnly).unwrap();
        let mut found: bool = false;
        let mut cand_domid = 0;
        for domid_str in xs.directory(XBTransaction::Null, "/local/domain".to_string()) {
            let name_path = format!("/local/domain/{}/name", domid_str);
            let candidate = xs.read(XBTransaction::Null, name_path);
            println!("Xenstore entry: [{}] {}", domid_str, candidate);
            if candidate == *domain_name {
                cand_domid = domid_str.parse::<u32>().unwrap();
                found = true;
            }
        }
        if !found {
            panic!("Cannot find domain {}", domain_name);
        }
        let xc = Xc::new().unwrap();
        let xen = Xen {
            xc: xc,
            dom_name: domain_name.clone(),
            domid: cand_domid,
        };
        println!("Initialized {:#?}", xen);
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
