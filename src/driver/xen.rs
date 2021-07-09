use std::convert::Infallible;
use std::convert::TryInto;
use std::error::Error;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::mem;
use std::num::TryFromIntError;

use libc::{PROT_READ, PROT_WRITE};
use nix::poll::PollFlags;
use nix::poll::{poll, PollFd};
use xenctrl::consts::{PAGE_SHIFT, PAGE_SIZE};
use xenctrl::error::XcError;
use xenctrl::RING_HAS_UNCONSUMED_REQUESTS;
use xenctrl::{XenControl, XenCr, XenEventType};
use xenevtchn::XenEventChannel;
use xenforeignmemory::{XenForeignMem, XenForeignMemoryError};
use xenstore_rs::{XBTransaction, Xs, XsOpenFlags};
use xenvmevent_sys::{
    vm_event_back_ring, vm_event_response_t, VM_EVENT_FLAG_VCPU_PAUSED, VM_EVENT_INTERFACE_VERSION,
};

use crate::api::events::{CrType, Event, EventType, InterceptType};
use crate::api::params::DriverInitParams;
use crate::api::registers::{Registers, SegmentReg, SystemTableReg, X86Registers};
use crate::api::{DriverType, Introspectable};

#[derive(Debug)]
pub struct Xen {
    xc: XenControl,
    xev: XenEventChannel,
    xen_fgn: XenForeignMem,
    dom_name: String,
    domid: u32,
    back_ring: vm_event_back_ring,
}

#[derive(thiserror::Error, Debug)]
pub enum XenDriverError {
    #[error("Xen driver requires a VM name parameter")]
    MissingVMName,
    #[error("failed to read xenstore entry {0}: {1}")]
    XenstoreReadError(String, IoError),
    #[error("event version mismatch: {0} <-> {1}")]
    EventVersionMismatch(u32, u32),
    #[error("failed to convert integer")]
    TryFromIntError(#[from] TryFromIntError),
    #[error("failed to convert integer")]
    InfallibleFromIntError(#[from] Infallible),
    #[error("IO error")]
    IoError(#[from] IoError),
    #[error("xenctrl error")]
    XcError(#[from] XcError),
    #[error("UNIX error")]
    NixError(#[from] nix::Error),
    #[error("xenforeignmemory error")]
    ForeignMemoryError(#[from] XenForeignMemoryError),
}

impl Xen {
    pub fn new(init_params: DriverInitParams) -> Result<Self, Box<dyn Error>> {
        let domain_name = init_params
            .common
            .ok_or(XenDriverError::MissingVMName)?
            .vm_name;
        // find domain name in xenstore
        let xs = Xs::new(XsOpenFlags::ReadOnly)?;
        let mut found: bool = false;
        let mut cand_domid = 0;
        for domid_str in xs.directory(XBTransaction::Null, "/local/domain")? {
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
                            return Err(Box::new(XenDriverError::XenstoreReadError(
                                name_path, error,
                            )))
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

        let mut xc = XenControl::new(None, None, 0)?;
        let (_ring_page, back_ring, remote_port) = xc.monitor_enable(cand_domid)?;
        let xev = XenEventChannel::new(cand_domid, remote_port)?;

        let xen_fgn = XenForeignMem::new()?;
        let xen = Xen {
            xc,
            xev,
            xen_fgn,
            dom_name: domain_name,
            domid: cand_domid,
            back_ring,
        };
        debug!("Initialized {:#?}", xen);
        Ok(xen)
    }
}

impl Introspectable for Xen {
    fn read_physical(
        &self,
        paddr: u64,
        buf: &mut [u8],
        bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        let mut cur_paddr: u64;
        let mut count_mut: u64 = buf.len() as u64;
        let mut buf_offset: u64 = 0;
        *bytes_read = 0;
        while count_mut > 0 {
            // compute new paddr
            cur_paddr = paddr + buf_offset;
            // get the current gfn
            let gfn = cur_paddr >> PAGE_SHIFT;
            let page_offset = u64::from(PAGE_SIZE - 1) & cur_paddr;
            // map gfn
            let page = self
                .xen_fgn
                .map(self.domid, PROT_READ, gfn)
                .map_err(XenDriverError::from)?;
            // determine how much we can read
            let read_len = if (page_offset + count_mut as u64) > u64::from(PAGE_SIZE) {
                u64::from(PAGE_SIZE) - page_offset
            } else {
                count_mut
            };

            // prepare offsets
            let buf_start = buf_offset as usize;
            let buf_end = (buf_offset + read_len) as usize;
            let page_start = page_offset as usize;
            let page_end = (page_offset + read_len) as usize;
            // do the read
            buf[buf_start..buf_end].copy_from_slice(&page[page_start..page_end]);
            // update loop variables
            count_mut -= read_len;
            buf_offset += read_len;
            *bytes_read += read_len;
            // unmap page
            self.xen_fgn.unmap(page).map_err(XenDriverError::from)?;
        }
        Ok(())
    }

    fn write_physical(&self, paddr: u64, buf: &[u8]) -> Result<(), Box<dyn Error>> {
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
            let page = self
                .xen_fgn
                .map(self.domid, PROT_WRITE, pfn)
                .map_err(XenDriverError::from)?;
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
            self.xen_fgn.unmap(page).map_err(XenDriverError::from)?;
        }
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        let max_gpfn = self
            .xc
            .domain_maximum_gpfn(self.domid)
            .map_err(XenDriverError::from)?;
        Ok(max_gpfn << PAGE_SHIFT)
    }

    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        let domain_info = self
            .xc
            .domain_getinfo(self.domid)
            .map_err(XenDriverError::from)?;
        Ok((domain_info.max_vcpu_id + 1)
            .try_into()
            .map_err(XenDriverError::from)?)
    }

    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let hvm_cpu = self
            .xc
            .domain_hvm_getcontext_partial(self.domid, vcpu)
            .map_err(XenDriverError::from)?;
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
                selector: hvm_cpu.cs_sel.try_into().map_err(XenDriverError::from)?,
            },
            ds: SegmentReg {
                base: hvm_cpu.ds_base,
                limit: hvm_cpu.ds_limit,
                selector: hvm_cpu.ds_sel.try_into().map_err(XenDriverError::from)?,
            },
            es: SegmentReg {
                base: hvm_cpu.es_base,
                limit: hvm_cpu.es_limit,
                selector: hvm_cpu.es_sel.try_into().map_err(XenDriverError::from)?,
            },
            fs: SegmentReg {
                base: hvm_cpu.fs_base,
                limit: hvm_cpu.fs_limit,
                selector: hvm_cpu.fs_sel.try_into().map_err(XenDriverError::from)?,
            },
            gs: SegmentReg {
                base: hvm_cpu.gs_base,
                limit: hvm_cpu.gs_limit,
                selector: hvm_cpu.gs_sel.try_into().map_err(XenDriverError::from)?,
            },
            ss: SegmentReg {
                base: hvm_cpu.ss_base,
                limit: hvm_cpu.ss_limit,
                selector: hvm_cpu.ss_sel.try_into().map_err(XenDriverError::from)?,
            },
            tr: SegmentReg {
                base: hvm_cpu.tr_base,
                limit: hvm_cpu.tr_limit,
                selector: hvm_cpu.tr_sel.try_into().map_err(XenDriverError::from)?,
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
                cpu.cs_sel = x86_registers
                    .cs
                    .selector
                    .try_into()
                    .map_err(XenDriverError::from)?;
                cpu.ds_sel = x86_registers
                    .ds
                    .selector
                    .try_into()
                    .map_err(XenDriverError::from)?;
                cpu.es_sel = x86_registers
                    .es
                    .selector
                    .try_into()
                    .map_err(XenDriverError::from)?;
                cpu.fs_sel = x86_registers
                    .fs
                    .selector
                    .try_into()
                    .map_err(XenDriverError::from)?;
                cpu.gs_sel = x86_registers
                    .gs
                    .selector
                    .try_into()
                    .map_err(XenDriverError::from)?;
                cpu.ss_sel = x86_registers
                    .ss
                    .selector
                    .try_into()
                    .map_err(XenDriverError::from)?;
                cpu.tr_sel = x86_registers
                    .tr
                    .selector
                    .try_into()
                    .map_err(XenDriverError::from)?;
            }
        }
        self.xc.domain_hvm_setcontext(
            self.domid,
            buffer,
            size.try_into().map_err(XenDriverError::from)?,
        )?;
        Ok(())
    }

    fn listen(&mut self, timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        let fd = self.xev.xenevtchn_fd().map_err(XenDriverError::from)?;
        let fd_struct = PollFd::new(fd, PollFlags::POLLIN | PollFlags::POLLERR);
        let mut fds = [fd_struct];
        let mut vcpu: u16 = 0;
        let mut event_type = unsafe { mem::MaybeUninit::<EventType>::zeroed().assume_init() };
        let poll_result = poll(&mut fds, timeout.try_into().map_err(XenDriverError::from)?)?;
        let mut pending_event_port = -1;
        if poll_result == 1 {
            pending_event_port = self.xev.xenevtchn_pending().map_err(XenDriverError::from)?;
            if pending_event_port != -1 {
                self.xev
                    .xenevtchn_unmask(
                        pending_event_port
                            .try_into()
                            .map_err(XenDriverError::from)?,
                    )
                    .map_err(XenDriverError::from)?;
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
                return Err(Box::new(XenDriverError::EventVersionMismatch(
                    req.version,
                    VM_EVENT_INTERFACE_VERSION,
                )));
            }
            let xen_event_type = (self.xc.get_event_type(req)).map_err(XenDriverError::from)?;
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
                XenEventType::Msr { msr_type, value } => EventType::Msr { msr_type, value },
                XenEventType::Breakpoint { insn_len, .. } => {
                    EventType::Breakpoint { gpa: 0, insn_len }
                }
                _ => unimplemented!(),
            };
            vcpu = req.vcpu_id.try_into().map_err(XenDriverError::from)?;
            let mut rsp =
                unsafe { mem::MaybeUninit::<vm_event_response_t>::zeroed().assume_init() };
            rsp.reason = req.reason;
            rsp.version = VM_EVENT_INTERFACE_VERSION;
            rsp.vcpu_id = req.vcpu_id;
            rsp.flags = req.flags & VM_EVENT_FLAG_VCPU_PAUSED;
            self.xc
                .put_response(&mut rsp, &mut self.back_ring)
                .map_err(XenDriverError::from)?;
        }
        self.xev.xenevtchn_notify().map_err(XenDriverError::from)?;
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
                    .monitor_write_ctrlreg(self.domid, xen_cr, enabled, true, true)
                    .map_err(XenDriverError::from)?)
            }
            InterceptType::Msr(micro_msr_type) => Ok(self
                .xc
                .monitor_mov_to_msr(self.domid, micro_msr_type, enabled)
                .map_err(XenDriverError::from)?),
            InterceptType::Breakpoint => Ok(self
                .xc
                .monitor_software_breakpoint(self.domid, enabled)
                .map_err(XenDriverError::from)?),
            _ => unimplemented!(),
        }
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("pause");
        Ok(self
            .xc
            .domain_pause(self.domid)
            .map_err(XenDriverError::from)?)
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("resume");
        Ok(self
            .xc
            .domain_unpause(self.domid)
            .map_err(XenDriverError::from)?)
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::Xen
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
