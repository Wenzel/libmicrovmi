use crate::api::{
    CrType, DriverInitParam, Event, EventReplyType, EventType, InterceptType, Introspectable,
    Registers, SegmentReg, SystemTableReg, X86Registers,
};
use libc::{PROT_READ, PROT_WRITE};
use nix::poll::PollFlags;
use nix::poll::{poll, PollFd};
use nix::sys::mman::munmap;
use std::convert::TryInto;
use std::error::Error;
use std::ffi::c_void;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::mem;
use xenctrl::consts::{PAGE_SHIFT, PAGE_SIZE};
use xenctrl::{
    XenControl, XenCr, XenEventType, RING_HAS_UNCONSUMED_REQUESTS,
    XEN_DOMCTL_DEBUG_OP_SINGLE_STEP_OFF, XEN_DOMCTL_DEBUG_OP_SINGLE_STEP_ON,
};
use xenevtchn::XenEventChannel;
use xenforeignmemory::XenForeignMem;
use xenstore_rs::{XBTransaction, Xs, XsOpenFlags};
use xenvmevent_sys::{
    vm_event_back_ring, vm_event_request_t, vm_event_response_t, vm_event_sring,
    VM_EVENT_FLAG_VCPU_PAUSED, VM_EVENT_INTERFACE_VERSION,
};

pub struct Xen {
    xc: XenControl,
    xev: XenEventChannel,
    xen_fgn: XenForeignMem,
    _dom_name: String,
    domid: u32,
    ring_page: *mut vm_event_sring,
    back_ring: vm_event_back_ring,
    evtchn_pollfd: PollFd,
    // VCPU -> vm_event_request_t
    vec_events: Vec<Option<vm_event_request_t>>,
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

        let mut xc = XenControl::new(None, None, 0).unwrap();
        let (ring_page, back_ring, remote_port) = xc
            .monitor_enable(cand_domid)
            .expect("Failed to map event ring page");

        let xev = XenEventChannel::new(cand_domid, remote_port).unwrap();
        let fd = xev.xenevtchn_fd().unwrap();
        let evtchn_pollfd = PollFd::new(fd, PollFlags::POLLIN | PollFlags::POLLERR);
        let xen_fgn = XenForeignMem::new().unwrap();

        let mut xen = Xen {
            xc,
            xev,
            xen_fgn,
            _dom_name: domain_name.to_string(),
            domid: cand_domid,
            ring_page,
            back_ring,
            evtchn_pollfd,
            vec_events: Vec::new(),
        };

        // enable singlestep monitoring
        // it will only intercept events when explicitely requested using
        // xc_domain_debug_control()
        xen.xc
            .monitor_singlestep(cand_domid, true)
            .unwrap_or_else(|_| panic!("Failed to enable singlestep monitoring"));

        let vcpu_count = xen.get_vcpu_count().expect("Failed to get VCPU count");
        // init vec events
        xen.vec_events.resize(vcpu_count.try_into().unwrap(), None);
        // TODO: vm_event_request_t (vm_event_st) doesn't derive Debug even when .derive_debug(true)
        // debug!("Initialized {:#?}", xen);
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

    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        let domain_info = self.xc.domain_getinfo(self.domid)?;
        let vcpu_count = (domain_info.max_vcpu_id + 1).try_into()?;
        Ok(vcpu_count)
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
            cr2: hvm_cpu.cr2,
            sysenter_cs: hvm_cpu.sysenter_cs,
            sysenter_esp: hvm_cpu.sysenter_esp,
            sysenter_eip: hvm_cpu.sysenter_eip,
            msr_efer: hvm_cpu.msr_efer,
            msr_star: hvm_cpu.msr_star,
            msr_lstar: hvm_cpu.msr_lstar,
            cs: SegmentReg {
                base: hvm_cpu.cs_base,
                limit: hvm_cpu.cs_limit,
                selector: hvm_cpu.cs_sel.try_into().unwrap(),
            },
            ds: SegmentReg {
                base: hvm_cpu.ds_base,
                limit: hvm_cpu.ds_limit,
                selector: hvm_cpu.ds_sel.try_into().unwrap(),
            },
            es: SegmentReg {
                base: hvm_cpu.es_base,
                limit: hvm_cpu.es_limit,
                selector: hvm_cpu.es_sel.try_into().unwrap(),
            },
            fs: SegmentReg {
                base: hvm_cpu.fs_base,
                limit: hvm_cpu.fs_limit,
                selector: hvm_cpu.fs_sel.try_into().unwrap(),
            },
            gs: SegmentReg {
                base: hvm_cpu.gs_base,
                limit: hvm_cpu.gs_limit,
                selector: hvm_cpu.gs_sel.try_into().unwrap(),
            },
            ss: SegmentReg {
                base: hvm_cpu.ss_base,
                limit: hvm_cpu.ss_limit,
                selector: hvm_cpu.ss_sel.try_into().unwrap(),
            },
            tr: SegmentReg {
                base: hvm_cpu.tr_base,
                limit: hvm_cpu.tr_limit,
                selector: hvm_cpu.tr_sel.try_into().unwrap(),
            },
            idt: SystemTableReg {
                base: hvm_cpu.idtr_base,
                limit: hvm_cpu.idtr_limit as u16,
            },
            gdt: SystemTableReg {
                base: hvm_cpu.gdtr_base,
                limit: hvm_cpu.gdtr_limit as u16,
            },
            ..Default::default()
        }))
    }

    fn write_registers(&self, vcpu: u16, reg: Registers) -> Result<(), Box<dyn Error>> {
        let (buffer, mut cpu, size) = self.xc.domain_hvm_getcontext(self.domid, vcpu)?;
        match reg {
            Registers::X86(x86_registers) => {
                cpu.rax = x86_registers.rax;
                cpu.rbx = x86_registers.rbx;
                cpu.rcx = x86_registers.rcx;
                cpu.rdx = x86_registers.rdx;
                cpu.rsi = x86_registers.rsi;
                cpu.rdi = x86_registers.rdi;
                cpu.rsp = x86_registers.rsp;
                cpu.rbp = x86_registers.rbp;
                cpu.r8 = x86_registers.r8;
                cpu.r9 = x86_registers.r9;
                cpu.r10 = x86_registers.r10;
                cpu.r11 = x86_registers.r11;
                cpu.r12 = x86_registers.r12;
                cpu.r13 = x86_registers.r13;
                cpu.r14 = x86_registers.r14;
                cpu.r15 = x86_registers.r15;
                cpu.rip = x86_registers.rip;
                cpu.rflags = x86_registers.rflags;
                cpu.cr0 = x86_registers.cr0;
                cpu.cr2 = x86_registers.cr2;
                cpu.cr3 = x86_registers.cr3;
                cpu.cr4 = x86_registers.cr4;
                cpu.sysenter_cs = x86_registers.sysenter_cs;
                cpu.sysenter_esp = x86_registers.sysenter_esp;
                cpu.sysenter_eip = x86_registers.sysenter_eip;
                cpu.msr_star = x86_registers.msr_star;
                cpu.msr_lstar = x86_registers.msr_lstar;
                cpu.msr_efer = x86_registers.msr_efer;
                cpu.cs_base = x86_registers.cs.base;
                cpu.ds_base = x86_registers.ds.base;
                cpu.es_base = x86_registers.es.base;
                cpu.fs_base = x86_registers.fs.base;
                cpu.gs_base = x86_registers.gs.base;
                cpu.ss_base = x86_registers.ss.base;
                cpu.tr_base = x86_registers.tr.base;
                cpu.cs_limit = x86_registers.cs.limit;
                cpu.ds_limit = x86_registers.ds.limit;
                cpu.es_limit = x86_registers.es.limit;
                cpu.fs_limit = x86_registers.fs.limit;
                cpu.gs_limit = x86_registers.gs.limit;
                cpu.ss_limit = x86_registers.ss.limit;
                cpu.tr_limit = x86_registers.tr.limit;
                cpu.cs_sel = x86_registers.cs.selector.try_into().unwrap();
                cpu.ds_sel = x86_registers.ds.selector.try_into().unwrap();
                cpu.es_sel = x86_registers.es.selector.try_into().unwrap();
                cpu.fs_sel = x86_registers.fs.selector.try_into().unwrap();
                cpu.gs_sel = x86_registers.gs.selector.try_into().unwrap();
                cpu.ss_sel = x86_registers.ss.selector.try_into().unwrap();
                cpu.tr_sel = x86_registers.tr.selector.try_into().unwrap();
            }
        }
        self.xc
            .domain_hvm_setcontext(self.domid, buffer, size.try_into().unwrap())?;
        Ok(())
    }

    fn listen(&mut self, timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        let mut fds: [PollFd; 1] = [self.evtchn_pollfd];
        let event: Option<Event> = match poll(&mut fds, timeout.try_into()?)? {
            0 => {
                // timeout. no file descriptors were ready
                None
            }
            -1 => {
                // failure
                return Err(Box::new(IoError::last_os_error()));
            }
            1 => {
                // event available
                match self.xev.xenevtchn_pending()? {
                    -1 => {
                        // no event channel port is pending
                        // TODO: Err
                        panic!("No event channel port is pending");
                    }
                    pending_event_port => {
                        let bind_port = self.xev.get_bind_port();
                        if pending_event_port != self.xev.get_bind_port() {
                            panic!(
                                "Event received for invalid port {}, expected port {}",
                                pending_event_port, bind_port
                            );
                        }
                        // unmask
                        self.xev.xenevtchn_unmask(pending_event_port.try_into()?)?;
                    }
                };
                let back_ring_ptr = &mut self.back_ring;
                if RING_HAS_UNCONSUMED_REQUESTS!(back_ring_ptr) == 0 {
                    None
                } else {
                    let req = self.xc.get_request(back_ring_ptr)?;
                    if req.version != VM_EVENT_INTERFACE_VERSION {
                        panic!("version mismatch");
                    }
                    let xen_event_type = (self.xc.get_event_type(req))?;
                    let vcpu: u32 = req.vcpu_id;
                    let event_type: EventType = match xen_event_type {
                        XenEventType::Cr { cr_type, new, old } => EventType::Cr {
                            cr_type: match cr_type {
                                XenCr::Cr0 => CrType::Cr0,
                                XenCr::Cr3 => CrType::Cr3,
                                XenCr::Cr4 => CrType::Cr4,
                            },
                            new,
                            old,
                        },
                        XenEventType::Msr { msr_type, value } => EventType::Msr { msr_type, value },
                        XenEventType::Breakpoint { insn_len, .. } => {
                            EventType::Breakpoint { gpa: 0, insn_len }
                        }
                        XenEventType::Singlestep { .. } => EventType::Singlestep,
                        _ => unimplemented!(),
                    };
                    // associate VCPU => vm_event_request_t
                    // to find it in reply_event()
                    let vcpu_index: usize = vcpu.try_into()?;
                    self.vec_events[vcpu_index] = Some(req);
                    Some(Event {
                        vcpu: vcpu.try_into()?,
                        kind: event_type,
                    })
                }
            }
            x => panic!("Unexpected poll return value {}", x),
        };

        Ok(event)
    }

    fn reply_event(
        &mut self,
        event: Event,
        reply_type: EventReplyType,
    ) -> Result<(), Box<dyn Error>> {
        let add_flags: u32 = match reply_type {
            EventReplyType::Continue => VM_EVENT_FLAG_VCPU_PAUSED,
        };
        // get the request back
        let vcpu_index: usize = event.vcpu.try_into()?;
        let req: vm_event_request_t = mem::replace(&mut self.vec_events[vcpu_index], None).unwrap();
        let mut rsp = vm_event_response_t {
            reason: req.reason,
            version: VM_EVENT_INTERFACE_VERSION,
            vcpu_id: req.vcpu_id,
            flags: req.flags & add_flags,
            ..Default::default()
        };
        self.xc.put_response(&mut rsp, &mut self.back_ring)?;
        Ok(self.xev.xenevtchn_notify()?)
    }

    fn toggle_intercept(
        &mut self,
        vcpu: u16,
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
            InterceptType::Msr(micro_msr_type) => {
                Ok(self
                    .xc
                    .monitor_mov_to_msr(self.domid, micro_msr_type, enabled)?)
            }
            InterceptType::Breakpoint => {
                Ok(self.xc.monitor_software_breakpoint(self.domid, enabled)?)
            }
            InterceptType::Singlestep => {
                let op: u32 = if enabled {
                    XEN_DOMCTL_DEBUG_OP_SINGLE_STEP_ON
                } else {
                    XEN_DOMCTL_DEBUG_OP_SINGLE_STEP_OFF
                };
                Ok(self
                    .xc
                    .domain_debug_control(self.domid, op, vcpu.try_into().unwrap())?)
            }
            _ => unimplemented!(),
        }
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("pause");
        // get domain info, check if already paused
        let dom_info = self.xc.domain_getinfo(self.domid)?;
        if dom_info.domid != self.domid {
            // TODO error
            panic!("Invalid domid: {}", dom_info.domid);
        }
        if dom_info.paused() == 1 {
            // already paused
            // nothing to do here
            debug!("already paused");
            return Ok(());
        }
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
        // ensure paused
        self.pause().expect("Failed to pause VM");
        // listen for remaining events to clear the ring
        let mut cleaned = false;
        while !cleaned {
            match self.listen(0).expect("Failed to listen for events") {
                None => cleaned = true,
                Some(e) => {
                    debug!("cleaning queue: {:?}", e);
                    // replying continue
                    self.reply_event(e, EventReplyType::Continue)
                        .unwrap_or_else(|_| panic!("Failed to reply for event"))
                }
            }
        }

        let vcpu_cpunt = self.get_vcpu_count().expect("Failed to get VCPU count");
        for vcpu in 0..vcpu_cpunt {
            debug!("disabling singlestep for VCPU {}", vcpu);
            self.toggle_intercept(vcpu, InterceptType::Singlestep, false)
                .unwrap_or_else(|_| panic!("Failed to disable singlestep on VCPU {}", vcpu));
        }

        self.xc
            .monitor_singlestep(self.domid, false)
            .unwrap_or_else(|_| panic!("Failed to disable singlestep monitoring"));
        // unmap
        debug!("unmapping ring buffer");
        unsafe {
            munmap(
                self.ring_page as *mut c_void,
                PAGE_SIZE.try_into().expect("Failed to convert to u32"),
            )
            .expect("Failed to unmap ring page");
        }

        self.xc
            .monitor_disable(self.domid)
            .expect("Failed to unmap event ring page");
        // resume
        self.resume().expect("Failed to resume VM");
    }
}
