use std::error::Error;
use std::mem;

use crate::api::{
    CrType, Event, EventType, InterceptType, Introspectable, Registers, SegmentReg, X86Registers, DriverInitParam,
};

use libc::PROT_READ;
use nix::poll::PollFlags;
use nix::poll::{poll, PollFd};
use std::convert::TryInto;
use xenctrl::consts::{PAGE_SHIFT, PAGE_SIZE};
use xenctrl::RING_HAS_UNCONSUMED_REQUESTS;
use xenctrl::{XenControl, XenCr, XenEventType};
use xenevtchn::XenEventChannel;
use xenforeignmemory::XenForeignMem;
use xenstore::{XBTransaction, Xs, XsOpenFlags};
use xenvmevent_sys::{
    vm_event_back_ring, vm_event_response_t, VM_EVENT_FLAG_VCPU_PAUSED, VM_EVENT_INTERFACE_VERSION,
};

#[derive(Debug)]
pub struct Xen {
    xc: XenControl,
    xev: XenEventChannel,
    xen_fgn: XenForeignMem,
    dom_name: String,
    domid: u32,
    back_ring: vm_event_back_ring,
}

impl Xen {
    pub fn new(domain_name: &str, _init_option: Option<DriverInitParam>) -> Self {
        debug!("init on {}", domain_name);
        // find domain name in xenstore
        let xs = Xs::new(XsOpenFlags::ReadOnly).unwrap();
        let mut found: bool = false;
        let mut cand_domid = 0;
        for domid_str in xs.directory(XBTransaction::Null, "/local/domain".to_string()) {
            let name_path = format!("/local/domain/{}/name", domid_str);
            let candidate = xs.read(XBTransaction::Null, name_path);
            debug!("Xenstore entry: [{}] {}", domid_str, candidate);
            if candidate == *domain_name {
                cand_domid = domid_str.parse::<u32>().unwrap();
                found = true;
            }
        }
        if !found {
            panic!("Cannot find domain {}", domain_name);
        }

        let mut xc = XenControl::new(None, None, 0).unwrap();
        let (_ring_page, back_ring, remote_port) = xc
            .monitor_enable(cand_domid)
            .expect("Failed to map event ring page");
        let xev = XenEventChannel::new(cand_domid, remote_port).unwrap();

        let xen_fgn = XenForeignMem::new().unwrap();
        let xen = Xen {
            xc,
            xev,
            xen_fgn,
            dom_name: domain_name.to_string(),
            domid: cand_domid,
            back_ring,
        };
        debug!("Initialized {:#?}", xen);
        xen
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
        }))
    }

    fn listen(&mut self, timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        let fd = self.xev.xenevtchn_fd()?;
        let fd_struct = PollFd::new(fd, PollFlags::POLLIN | PollFlags::POLLERR);
        let mut fds = [fd_struct];
        let mut vcpu: u16 = 0;
        let mut event_type = unsafe { mem::MaybeUninit::<EventType>::zeroed().assume_init() };
        let poll_result = poll(&mut fds, timeout.try_into().unwrap()).unwrap();
        let mut pending_event_port = -1;
        if poll_result == 1 {
            pending_event_port = self.xev.xenevtchn_pending()?;
            if pending_event_port != -1 {
                self.xev
                    .xenevtchn_unmask(pending_event_port.try_into().unwrap())?;
            }
        }
        let back_ring_ptr = &mut self.back_ring;
        let mut flag = false;
        if poll_result > 0
            && self.xev.get_bind_port() == pending_event_port
            && RING_HAS_UNCONSUMED_REQUESTS!(back_ring_ptr) != 0
        {
            flag = true;
            let req = self.xc.get_request(back_ring_ptr)?;
            if req.version != VM_EVENT_INTERFACE_VERSION {
                panic!("version mismatch");
            }
            let xen_event_type = (self.xc.get_event_type(req)).unwrap();
            event_type = match xen_event_type {
                XenEventType::Cr { cr_type, new, old } => EventType::Cr {
                    cr_type: match cr_type {
                        XenCr::Cr0 => CrType::Cr0,
                        XenCr::Cr3 => CrType::Cr3,
                        XenCr::Cr4 => CrType::Cr4,
                    },
                    new,
                    old,
                },
                _ => unimplemented!(),
            };
            vcpu = req.vcpu_id.try_into().unwrap();
            let mut rsp =
                unsafe { mem::MaybeUninit::<vm_event_response_t>::zeroed().assume_init() };
            rsp.reason = req.reason;
            rsp.version = VM_EVENT_INTERFACE_VERSION;
            rsp.vcpu_id = req.vcpu_id;
            rsp.flags = req.flags & VM_EVENT_FLAG_VCPU_PAUSED;
            self.xc.put_response(&mut rsp, &mut self.back_ring)?;
        }
        self.xev.xenevtchn_notify()?;
        if flag {
            Ok(Some(Event {
                vcpu,
                kind: event_type,
            }))
        } else {
            Ok(None)
        }
    }

    fn toggle_intercept(
        &mut self,
        _vcpu: u16,
        intercept_type: InterceptType,
        enabled: bool,
    ) -> Result<(), Box<dyn Error>> {
        match intercept_type {
            InterceptType::Cr(micro_cr_type) => {
                let xen_cr = match micro_cr_type {
                    CrType::Cr0 => XenCr::Cr0,
                    CrType::Cr3 => XenCr::Cr3,
                    CrType::Cr4 => XenCr::Cr4,
                };
                Ok(self
                    .xc
                    .monitor_write_ctrlreg(self.domid, xen_cr, enabled, true, true)?)
            }
            _ => unimplemented!(),
        }
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
        debug!("Closing Xen driver");
        self.xc
            .monitor_disable(self.domid)
            .expect("Failed to unmap event ring page");
    }
}
