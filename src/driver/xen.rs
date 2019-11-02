use std::error::Error;
use crate::api;
use xenctrl::XenControl;
use xenctrl::consts::{PAGE_SHIFT, PAGE_SIZE};
use xenstore::{Xs, XBTransaction, XsOpenFlags};
use libc::PROT_READ;

// unit struct
#[derive(Debug)]
pub struct Xen {
    xc: XenControl,
    xen_fgn: xenforeignmemory::XenForeignMem,
    dom_name: String,
    domid: u32,
}

impl Xen {

    pub fn new(domain_name: &str) -> Self {
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
        let xc = XenControl::new(None, None, 0).unwrap();
        let xen_fgn = xenforeignmemory::XenForeignMem::new().unwrap();
        let xen = Xen {
            xc,
            xen_fgn,
            dom_name: domain_name.to_string(),
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

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),Box<dyn Error>> {
        let mut cur_paddr: u64;
        let mut offset: u64 = 0;
        let mut count_mut: u64 = buf.len() as u64;
        let mut buf_offset: u64 = 0;
        while count_mut > 0 {
            // compute new paddr
            cur_paddr = paddr + offset;
            // get the current gfn
            let gfn = cur_paddr >> PAGE_SHIFT;
            offset = u64::from(PAGE_SIZE - 1) & cur_paddr;
            // map gfn
            let page = self.xen_fgn.map(self.domid, PROT_READ, gfn)?;
            // determine how much we can read
            let read_len = if (offset + count_mut as u64) > u64::from(PAGE_SIZE) {
                u64::from(PAGE_SIZE) - offset
            } else {
                buf.len() as u64
            };

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

    fn get_max_physical_addr(&self) -> Result<u64,Box<dyn Error>> {
        let max_gpfn = self.xc.domain_maximum_gpfn(self.domid)?;
        Ok(max_gpfn << PAGE_SHIFT)
    }

    fn pause(&mut self) -> Result<(),Box<dyn Error>> {
        println!("Xen driver pause");
        Ok(self.xc.domain_pause(self.domid)?)
    }

    fn resume(&mut self) -> Result<(),Box<dyn Error>> {
        println!("Xen driver resume");
        Ok(self.xc.domain_unpause(self.domid)?)
    }

}

impl Drop for Xen {
    fn drop(&mut self) {
        self.close();
    }
}
