extern crate xenctrl;
extern crate xenstore;
extern crate xenforeignmemory;
extern crate libc;
use crate::api;
use xenctrl::Xc;
use xenstore::{Xs, XBTransaction, XsOpenFlags};
use libc::PROT_READ;

// unit struct
#[derive(Debug)]
pub struct Xen {
    xc: Xc,
    xen_fgn: xenforeignmemory::XenForeignMem,
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
        let xen_fgn = xenforeignmemory::XenForeignMem::new().unwrap();
        let xen = Xen {
            xc: xc,
            xen_fgn: xen_fgn,
            dom_name: domain_name.clone(),
            domid: cand_domid,
        };
        println!("Initialized {:#?}", xen);
        xen
    }

    fn close(&mut self) {
        println!("Xen driver close");
    }
}

impl api::Introspectable for Xen {

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),&str> {
        let mut cur_paddr: u64;
        let mut offset: u64 = 0;
        let mut read_len: u64;
        let mut count_mut: u64 = buf.len() as u64;
        let mut buf_offset: u64 = 0;
        while count_mut > 0 {
            // compute new paddr
            cur_paddr = paddr + offset;
            // get the current gfn
            let gfn = cur_paddr >> xenctrl::PAGE_SHIFT;
            offset = ((xenctrl::PAGE_SIZE - 1) as u64) & cur_paddr;
            // map gfn
            let page = self.xen_fgn.map(self.domid, PROT_READ, gfn).unwrap();
            // determine how much we can read
            if (offset + count_mut as u64) > xenctrl::PAGE_SIZE as u64 {
                read_len = (xenctrl::PAGE_SIZE as u64) - offset;
            } else {
                read_len = buf.len() as u64;
            }

            // do the read
            buf[buf_offset as usize..].copy_from_slice(&page[..read_len as usize]);
            // update loop variables
            count_mut -= read_len;
            buf_offset += read_len;
            // unmap page
            self.xen_fgn.unmap(page).unwrap();
        }
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64,&str> {
        let max_gpfn = self.xc.domain_maximum_gpfn(self.domid).unwrap();
        Ok(max_gpfn << xenctrl::PAGE_SHIFT)
    }

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
