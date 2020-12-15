use crate::api::{
    DriverInitParam, Introspectable, Registers, SegmentReg, SystemTableReg, X86Registers,
};
use libc::{PROT_READ, PROT_WRITE};
use std::error::Error;
use std::io::ErrorKind;
use xenctrl::consts::{PAGE_SHIFT, PAGE_SIZE};
use xenctrl::XenControl;
use xenstore_rs::{XBTransaction, Xs, XsOpenFlags};

// unit struct
#[derive(Debug)]
pub struct Xen {
    xc: XenControl,
    xen_fgn: xenforeignmemory::XenForeignMem,
    dom_name: String,
    domid: u32,
}

impl Xen {
    pub fn new(domain_name: &str, _init_option: Option<DriverInitParam>) -> Self {
        debug!("init on {}", domain_name);
        // find domain name in xenstore
        let xs = Xs::new(XsOpenFlags::ReadOnly).unwrap();
        let mut found: bool = false;
        let mut cand_domid = 0;
        for domid_str in xs
            .directory(XBTransaction::Null, "/local/domain")
            .expect("Failed to enumerate xenstore /local/domain directory")
        {
            let name_path = format!("/local/domain/{}/name", domid_str);
            let candidate = match xs.read(XBTransaction::Null, &name_path) {
                Ok(candidate) => candidate,
                Err(error) => {
                    match error.kind() {
                        ErrorKind::PermissionDenied => {
                            // the domain has access to Xenstore only to a subset of the ids available
                            // we should continue
                            debug!("failed to read xenstore entry {}", name_path);
                            continue;
                        }
                        _ => {
                            panic!("Failed to read xenstore entry {}: {}", name_path, error);
                        }
                    }
                }
            };
            debug!("Xenstore entry: [{}] {}", domid_str, candidate);
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
        debug!("Initialized {:#?}", xen);
        xen
    }

    fn close(&mut self) {
        debug!("close");
    }
}

impl Introspectable for Xen {
    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
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

    fn write_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        let mut phys_address: u64;
        let mut offset: u64;
        let mut count_mut: u64 = buf.len() as u64;
        let mut buf_offset: u64 = 0;
        while count_mut > 0 {
            // compute new paddr
            phys_address = paddr + buf_offset;
            // get the current pfn
            let pfn = phys_address >> PAGE_SHIFT;
            offset = u64::from(PAGE_SIZE - 1) & phys_address;
            // map pfn
            let page = self.xen_fgn.map(self.domid, PROT_WRITE, pfn)?;
            // determine how much we can write
            let write_len = if (offset + count_mut as u64) > u64::from(PAGE_SIZE) {
                u64::from(PAGE_SIZE) - offset
            } else {
                count_mut as u64
            };

            // do the write
            page[offset as usize..write_len as usize].copy_from_slice(&buf[buf_offset as usize..]);
            // update loop variables
            count_mut -= write_len;
            buf_offset += write_len;
            // unmap page
            self.xen_fgn.unmap(page).unwrap();
        }
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        let max_gpfn = self.xc.domain_maximum_gpfn(self.domid)?;
        Ok(max_gpfn << PAGE_SHIFT)
    }

    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let hvm_cpu = self.xc.domain_hvm_getcontext_partial(self.domid, vcpu)?;
        // TODO: hardcoded for x86 for now
        Ok(Registers::X86(X86Registers {
            rax: hvm_cpu.rax,
            rbx: hvm_cpu.rbx,
            rcx: hvm_cpu.rcx,
            rdx: hvm_cpu.rdx,
            rsi: hvm_cpu.rsi,
            rdi: hvm_cpu.rdi,
            rsp: hvm_cpu.rsp,
            rbp: hvm_cpu.rbp,
            r8: hvm_cpu.r8,
            r9: hvm_cpu.r9,
            r10: hvm_cpu.r10,
            r11: hvm_cpu.r11,
            r12: hvm_cpu.r12,
            r13: hvm_cpu.r13,
            r14: hvm_cpu.r14,
            r15: hvm_cpu.r15,
            rip: hvm_cpu.rip,
            rflags: hvm_cpu.rflags,
            cr0: hvm_cpu.cr0,
            cr3: hvm_cpu.cr3,
            cr4: hvm_cpu.cr4,
            cr2: 0,
            sysenter_cs: 0,
            sysenter_esp: 0,
            sysenter_eip: 0,
            msr_efer: 0,
            msr_star: 0,
            msr_lstar: 0,
            efer: 0,
            apic_base: 0,
            cs: SegmentReg {
                base: 0,
                limit: 0,
                selector: 0,
            },
            ds: SegmentReg {
                base: 0,
                limit: 0,
                selector: 0,
            },
            es: SegmentReg {
                base: 0,
                limit: 0,
                selector: 0,
            },
            fs: SegmentReg {
                base: 0,
                limit: 0,
                selector: 0,
            },
            gs: SegmentReg {
                base: 0,
                limit: 0,
                selector: 0,
            },
            ss: SegmentReg {
                base: 0,
                limit: 0,
                selector: 0,
            },
            tr: SegmentReg {
                base: 0,
                limit: 0,
                selector: 0,
            },
            ldt: SegmentReg {
                base: 0,
                limit: 0,
                selector: 0,
            },
            idt: SystemTableReg {
                base: hvm_cpu.idtr_base,
                limit: hvm_cpu.idtr_limit as u16,
            },
            gdt: SystemTableReg {
                base: hvm_cpu.gdtr_base,
                limit: hvm_cpu.gdtr_limit as u16,
            },
        }))
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("pause");
        Ok(self.xc.domain_pause(self.domid)?)
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("resume");
        Ok(self.xc.domain_unpause(self.domid)?)
    }
}

impl Drop for Xen {
    fn drop(&mut self) {
        self.close();
    }
}
