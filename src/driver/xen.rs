use crate::api::{
    Access, CrType, DriverInitParam, Event, EventType, InterceptType, Introspectable, Registers,
    SegmentReg, SystemTableReg, X86Registers,
};
use std::convert::{From, TryFrom};
use std::error::Error;
use std::mem;

use libc::{PROT_READ, PROT_WRITE};
use nix::poll::PollFlags;
use nix::poll::{poll, PollFd};
use std::convert::TryInto;
use xenctrl::consts::{PAGE_SHIFT, PAGE_SIZE};
use xenctrl::RING_HAS_UNCONSUMED_REQUESTS;
use xenctrl::{XenCr, XenEventType, XenIntrospectable, XenPageAccess};
use xenevtchn::EventChannelSetup;
use xenforeignmemory::XenForeignMemoryIntrospectable;
use xenstore::{XBTransaction, XsIntrospectable, XsOpenFlags};
use xenvmevent_sys::{
    vm_event_back_ring, vm_event_response_t, VM_EVENT_FLAG_VCPU_PAUSED, VM_EVENT_INTERFACE_VERSION,
};

impl TryFrom<Access> for XenPageAccess {
    type Error = &'static str;
    fn try_from(access: Access) -> Result<Self, Self::Error> {
        match access {
            Access::NIL => Ok(XenPageAccess::NIL),
            Access::R => Ok(XenPageAccess::R),
            Access::W => Ok(XenPageAccess::W),
            Access::RW => Ok(XenPageAccess::RW),
            Access::X => Ok(XenPageAccess::X),
            Access::RX => Ok(XenPageAccess::RX),
            Access::WX => Ok(XenPageAccess::WX),
            Access::RWX => Ok(XenPageAccess::RWX),
            _ => Err("invalid access value"),
        }
    }
}

impl From<XenPageAccess> for Access {
    fn from(access: XenPageAccess) -> Self {
        match access {
            XenPageAccess::NIL => Access::NIL,
            XenPageAccess::R => Access::R,
            XenPageAccess::W => Access::W,
            XenPageAccess::RW => Access::RW,
            XenPageAccess::X => Access::X,
            XenPageAccess::RX => Access::RX,
            XenPageAccess::WX => Access::WX,
            XenPageAccess::RWX => Access::RWX,
        }
    }
}

#[derive(Debug)]
pub struct Xen<
    T: XenIntrospectable,
    U: EventChannelSetup,
    V: XenForeignMemoryIntrospectable,
    W: XsIntrospectable,
> {
    xc: T,
    xev: U,
    xen_fgn: V,
    xs: W,
    dom_name: String,
    domid: u32,
    back_ring: vm_event_back_ring,
}

impl<
        T: XenIntrospectable,
        U: EventChannelSetup,
        V: XenForeignMemoryIntrospectable,
        W: XsIntrospectable,
    > Xen<T, U, V, W>
{
    pub fn new(
        domain_name: &str,
        mut xc: T,
        mut xev: U,
        mut xen_fgn: V,
        mut xs: W,
        _init_option: Option<DriverInitParam>,
    ) -> Result<Self, Box<dyn Error>> {
        debug!("init on {}", domain_name);
        xs.init(XsOpenFlags::ReadOnly)?;
        // find domain name in xenstore
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

        xc.init(None, None, 0)?;
        let (_ring_page, back_ring, remote_port) = xc
            .monitor_enable(cand_domid)
            .expect("Failed to map event ring page");
        xev.init(cand_domid, remote_port)?;
        xen_fgn.init()?;
        let xen = Xen {
            xc,
            xev,
            xen_fgn,
            xs,
            dom_name: domain_name.to_string(),
            domid: cand_domid,
            back_ring,
        };
        debug!("Initialized {:#?}", xen);
        Ok(xen)
    }
}

impl<
        T: XenIntrospectable,
        U: EventChannelSetup,
        V: XenForeignMemoryIntrospectable,
        W: XsIntrospectable,
    > Introspectable for Xen<T, U, V, W>
{
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
            self.xen_fgn.unmap(page)?;
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
            self.xen_fgn.unmap(page)?;
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
            let xen_event_type = (self.xc.get_event_type(req))?;
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
                XenEventType::Breakpoint { gpa, insn_len } => {
                    EventType::Breakpoint { gpa, insn_len }
                }
                XenEventType::Pagefault { gva, gpa, access } => EventType::Pagefault {
                    gva,
                    gpa,
                    access: access.into(),
                },
                XenEventType::Singlestep { gpa } => EventType::Singlestep { gpa },
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

    fn get_page_access(&self, paddr: u64) -> Result<Access, Box<dyn Error>> {
        let access = self.xc.get_mem_access(self.domid, paddr >> PAGE_SHIFT)?;
        Ok(access.into())
    }

    fn set_page_access(&self, paddr: u64, access: Access) -> Result<(), Box<dyn Error>> {
        Ok(self
            .xc
            .set_mem_access(self.domid, access.try_into().unwrap(), paddr >> PAGE_SHIFT)?)
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
            InterceptType::Msr(micro_msr_type) => {
                Ok(self
                    .xc
                    .monitor_mov_to_msr(self.domid, micro_msr_type, enabled)?)
            }
            InterceptType::Breakpoint => {
                Ok(self.xc.monitor_software_breakpoint(self.domid, enabled)?)
            }
            InterceptType::Pagefault => Ok(()),
            InterceptType::Singlestep => Ok(self.xc.monitor_singlestep(self.domid, enabled)?),
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

impl<
        T: XenIntrospectable,
        U: EventChannelSetup,
        V: XenForeignMemoryIntrospectable,
        W: XsIntrospectable,
    > Drop for Xen<T, U, V, W>
{
    fn drop(&mut self) {
        debug!("Closing Xen driver");
        self.xc
            .monitor_disable(self.domid)
            .expect("Failed to unmap event ring page");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate::eq;
    use std::fmt::{Debug, Formatter};
    use std::os::raw::{c_int, c_uint};
    use xenctrl::consts::{PAGE_SHIFT, PAGE_SIZE};
    use xenctrl::{
        hvm_hw_cpu, vm_event_back_ring, vm_event_request_t, vm_event_response_t, vm_event_sring,
        xentoollog_logger, XenCr, XenEventType, XenPageAccess,
    };
    use xenevtchn::{evtchn_port_t, xenevtchn_port_or_error_t};
    use xenstore::{XBTransaction, XsOpenFlags};

    #[test]
    fn test_fail_to_create_xen_driver_if_xencontrol_init_returns_error() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        xencontrol_mock
            .expect_init()
            .returning(|_, _, _| Err(xenctrl::error::XcError::new("some error")));
        xenevtchn_mock.expect_init().returning(|_, _| Ok(()));
        xenforeignmemory_mock.expect_init().returning(|| Ok(()));
        xenstore_mock.expect_init().returning(|_| Ok(()));
        let mut vec: Vec<String> = Vec::new();
        let domain_name = "some_vm";
        let dom0 = "0";
        let string_dir: String = String::from(dom0);
        let string_read: String = String::from(domain_name);
        vec.push(string_dir);
        xenstore_mock
            .expect_directory()
            .return_once(move |_, _| vec);
        xenstore_mock
            .expect_read()
            .return_once(move |_, _| string_read);

        let result = Xen::new(
            &domain_name,
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        );

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_fail_to_create_xen_driver_if_xenforeignmemory_init_returns_error() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        xencontrol_mock.expect_init().returning(|_, _, _| Ok(()));
        xenevtchn_mock.expect_init().returning(|_, _| Ok(()));
        xenforeignmemory_mock.expect_init().returning(|| {
            Err(failure::Error::from_boxed_compat(Box::new(
                std::io::Error::new(std::io::ErrorKind::Other, "something went wrong"),
            )))
        });
        xenstore_mock.expect_init().returning(|_| Ok(()));
        let mut vec: Vec<String> = Vec::new();
        let domain_name = "some_vm";
        let dom0 = "0";
        let string_dir: String = String::from(dom0);
        vec.push(string_dir);
        let string_read: String = String::from(domain_name);
        xenstore_mock
            .expect_directory()
            .return_once(move |_, _| vec);
        xenstore_mock
            .expect_read()
            .return_once(move |_, _| string_read);
        xencontrol_mock
            .expect_monitor_enable()
            .return_once(move |_| {
                let sring: vm_event_sring = Default::default();
                let bring: vm_event_back_ring = Default::default();
                Ok((sring, bring, 0))
            });

        let result = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        );

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_fail_to_create_xen_driver_if_xenstore_init_returns_error() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        xencontrol_mock.expect_init().returning(|_, _, _| Ok(()));
        xenevtchn_mock.expect_init().returning(|_, _| Ok(()));
        xenforeignmemory_mock.expect_init().returning(|| Ok(()));
        xenstore_mock.expect_init().returning(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });

        let result = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        );

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_fail_to_create_xen_driver_if_xenevtchn_init_returns_error() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        xencontrol_mock.expect_init().returning(|_, _, _| Ok(()));
        xenevtchn_mock.expect_init().returning(|_, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
        xenforeignmemory_mock.expect_init().returning(|| Ok(()));
        xenstore_mock.expect_init().returning(|_| Ok(()));
        let mut vec: Vec<String> = Vec::new();
        let domain_name = "some_vm";
        let dom0 = "0";
        let string_dir: String = String::from(dom0);
        let string_read: String = String::from(domain_name);
        vec.push(string_dir);
        xenstore_mock
            .expect_directory()
            .return_once(move |_, _| vec);
        xenstore_mock
            .expect_read()
            .return_once(move |_, _| string_read);
        xencontrol_mock
            .expect_monitor_enable()
            .return_once(move |_| {
                let sring: vm_event_sring = Default::default();
                let bring: vm_event_back_ring = Default::default();
                Ok((sring, bring, 0))
            });

        let result = Xen::new(
            &domain_name,
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        );

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    fn setup(
        xencontrol_mock: &mut MockXenControl,
        xenevtchn_mock: &mut MockXenEventChannel,
        xenforeignmemory_mock: &mut MockXenForeignMem,
        xenstore_mock: &mut MockXs,
    ) {
        xencontrol_mock.expect_init().returning(|_, _, _| Ok(()));
        xenevtchn_mock.expect_init().returning(|_, _| Ok(()));
        xenforeignmemory_mock.expect_init().returning(|| Ok(()));
        xenstore_mock.expect_init().returning(|_| Ok(()));
        let mut vec: Vec<String> = Vec::new();
        let domain_name = "some_vm";
        let dom0 = "0";
        let string_dir: String = String::from(dom0);
        let string_read: String = String::from(domain_name);
        vec.push(string_dir);
        xenstore_mock
            .expect_directory()
            .return_once(move |_, _| vec);
        xenstore_mock
            .expect_read()
            .return_once(move |_, _| string_read);
        xencontrol_mock
            .expect_monitor_enable()
            .return_once(move |_| {
                let sring: vm_event_sring = Default::default();
                let bring: vm_event_back_ring = Default::default();
                Ok((sring, bring, 0))
            });
        xencontrol_mock
            .expect_monitor_disable()
            .return_once(|_| Ok(()));
    }

    #[test]
    fn test_xen_driver_created_successfully() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );

        let result = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        );

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_read_physical_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        static mut BUF: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];
        unsafe {
            xenforeignmemory_mock
                .expect_map()
                .return_once(move |_, _, _| Ok(&mut BUF));
        }
        xenforeignmemory_mock.expect_unmap().returning(|_| Ok(()));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;
        let mut buffer = [0; PAGE_SIZE as usize];

        let result = xen.read_physical(paddr, &mut buffer);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_read_physical_fails_if_xen_foreignmemory_map_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xenforeignmemory_mock.expect_map().returning(|_, _, _| {
            Err(failure::Error::from_boxed_compat(Box::new(
                std::io::Error::new(std::io::ErrorKind::Other, "something went wrong"),
            )))
        });
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;
        let mut buffer = [0; PAGE_SIZE as usize];

        let result = xen.read_physical(paddr, &mut buffer);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_read_physical_fails_if_xen_foreignmemory_unmap_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        static mut BUF: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];
        unsafe {
            xenforeignmemory_mock
                .expect_map()
                .return_once(move |_, _, _| Ok(&mut BUF));
        }
        xenforeignmemory_mock.expect_unmap().returning(|_| {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            )))
        });
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;
        let mut buffer = [0; PAGE_SIZE as usize];

        let result = xen.read_physical(paddr, &mut buffer);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_read_registers_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        let cpu: hvm_hw_cpu = Default::default();
        xencontrol_mock
            .expect_domain_hvm_getcontext_partial()
            .returning(move |_, _| Ok(cpu));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.read_registers(vcpu);

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn test_read_registers_fails_if_domain_hvm_getcontext_partial_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_domain_hvm_getcontext_partial()
            .returning(move |_, _| Err(xenctrl::error::XcError::new("some error")));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.read_registers(vcpu);

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn test_write_registers_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        let cpu: hvm_hw_cpu = Default::default();
        let size = 1024;
        xencontrol_mock
            .expect_domain_hvm_getcontext()
            .return_once(move |_, _| {
                let buf: *mut c_uint = std::ptr::null_mut();
                Ok((buf, cpu, size))
            });
        xencontrol_mock
            .expect_domain_hvm_setcontext()
            .returning(move |_, _, _| Ok(()));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;
        let x86_register_struct: X86Registers = Default::default();

        let result = xen.write_registers(vcpu, Registers::X86(x86_register_struct));

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn test_write_registers_fails_if_domain_hvm_getcontext_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_domain_hvm_getcontext()
            .returning(move |_, _| Err(xenctrl::error::XcError::new("some error")));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;
        let x86_register_struct: X86Registers = Default::default();

        let result = xen.write_registers(vcpu, Registers::X86(x86_register_struct));

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn test_write_registers_fails_if_domain_hvm_setcontext_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        let cpu: hvm_hw_cpu = Default::default();
        let size = 1024;
        xencontrol_mock
            .expect_domain_hvm_getcontext()
            .return_once(move |_, _| {
                let buf: *mut c_uint = std::ptr::null_mut();
                Ok((buf, cpu, size))
            });
        xencontrol_mock
            .expect_domain_hvm_setcontext()
            .returning(move |_, _, _| Err(xenctrl::error::XcError::new("some error")));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;
        let x86_register_struct: X86Registers = Default::default();

        let result = xen.write_registers(vcpu, Registers::X86(x86_register_struct));

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn test_write_physical_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        static mut BUF: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];
        unsafe {
            xenforeignmemory_mock
                .expect_map()
                .return_once(move |_, _, _| Ok(&mut BUF));
        }
        xenforeignmemory_mock.expect_unmap().returning(|_| Ok(()));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;
        let mut buffer = [0; PAGE_SIZE as usize];

        let result = xen.write_physical(paddr, &mut buffer);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    /*#[test]
    fn test_write_physical_fails_if_xen_foreignmemory_map_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(&mut xencontrol_mock, &mut xenevtchn_mock, &mut xenforeignmemory_mock, &mut xenstore_mock);
        static mut buf: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];
        unsafe {xenforeignmemory_mock.expect_map().return_once(move |_, _, _| Ok(&mut buf));}
        xenforeignmemory_mock.expect_unmap().returning(|_| Ok(()));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        ).expect("Failed to create driver");
        let paddr =0;
        let mut buffer = [0;PAGE_SIZE as usize];
        let result = xen.write_physical(paddr, &mut buffer);
        assert!(result.is_ok(), "Expected ok, got error instead!");
    }*/

    #[test]
    fn test_write_physical_fails_if_xen_foreignmemory_unmap_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        static mut BUF: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];
        unsafe {
            xenforeignmemory_mock
                .expect_map()
                .return_once(move |_, _, _| Ok(&mut BUF));
        }
        xenforeignmemory_mock.expect_unmap().returning(|_| {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            )))
        });
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;
        let mut buffer = [0; PAGE_SIZE as usize];

        let result = xen.write_physical(paddr, &mut buffer);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn get_maximum_physical_address_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        let max_gfn = 1;
        xencontrol_mock
            .expect_domain_maximum_gpfn()
            .returning(move |_| Ok(max_gfn));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");

        let max_addr = xen.get_max_physical_addr().unwrap();

        assert_eq!(max_addr, max_gfn << PAGE_SHIFT);
    }

    #[test]
    fn get_maximum_physical_address_fails_if_xenctrl_domain_maximum_gpfn_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_domain_maximum_gpfn()
            .returning(|_| Err(xenctrl::error::XcError::new("some error")));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");

        let result = xen.get_max_physical_addr();

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_pause_domain_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock.expect_domain_pause().returning(|_| Ok(()));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");

        let result = xen.pause();

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn test_pause_domain_fails_if_xencontrol_domain_pause_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_domain_pause()
            .returning(|_| Err(xenctrl::error::XcError::new("some error")));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");

        let result = xen.pause();

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn test_resume_domain_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_domain_unpause()
            .returning(|_| Ok(()));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");

        let result = xen.resume();

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn test_resume_domain_fails_if_xencontrol_domain_unpause_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_domain_unpause()
            .returning(|_| Err(xenctrl::error::XcError::new("some error")));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");

        let result = xen.resume();

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn test_get_page_access_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_get_mem_access()
            .returning(move |_, _| Ok(XenPageAccess::R));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;

        let access = xen.get_page_access(paddr).unwrap();

        assert_eq!(Access::R, access);
    }

    #[test]
    fn test_get_page_access_fails_if_xencontrol_get_mem_access_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_get_mem_access()
            .returning(|_, _| Err(xenctrl::error::XcError::new("some error")));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;

        let result = xen.get_page_access(paddr);

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn test_set_page_access_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_set_mem_access()
            .returning(move |_, _, _| Ok(()));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;

        let result = xen.set_page_access(paddr, Access::R);

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn test_set_page_access_fails_if_xencontrol_set_mem_access_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_set_mem_access()
            .returning(|_, _, _| Err(xenctrl::error::XcError::new("some error")));
        let xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let paddr = 0;

        let result = xen.set_page_access(paddr, Access::R);

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn test_listen_events_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        let fd = 1;
        xenevtchn_mock
            .expect_xenevtchn_fd()
            .return_once(move || Ok(fd));
        let port = 56;
        xenevtchn_mock
            .expect_xenevtchn_pending()
            .return_once(move || Ok(port));
        xenevtchn_mock
            .expect_xenevtchn_unmask()
            .with(eq(port as u32))
            .returning(|_| Ok(()));
        xenevtchn_mock
            .expect_get_bind_port()
            .return_once(move || port);
        let request: vm_event_request_t = Default::default();
        xencontrol_mock
            .expect_get_request()
            .return_once(move |_| Ok(request));
        let cr_type = XenCr::Cr3;
        let new = 1;
        let old = 2;
        xencontrol_mock
            .expect_get_event_type()
            .return_once(move |_| Ok(XenEventType::Cr { cr_type, new, old }));
        xencontrol_mock
            .expect_put_response()
            .returning(|_, _| Ok(()));
        xenevtchn_mock
            .expect_xenevtchn_notify()
            .returning(|| Ok(()));
        let timeout = 1000;
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");

        let result = xen.listen(timeout);

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn toggle_cr_intercept_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_monitor_write_ctrlreg()
            .returning(|_, _, _, _, _| Ok(()));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.toggle_intercept(vcpu, InterceptType::Cr(CrType::Cr3), true);

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn toggle_cr_intercept_fails_if_monitor_write_ctrlreg_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_monitor_write_ctrlreg()
            .returning(|_, _, _, _, _| Err(xenctrl::error::XcError::new("some error")));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.toggle_intercept(vcpu, InterceptType::Cr(CrType::Cr3), true);

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn toggle_msr_intercept_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_monitor_mov_to_msr()
            .returning(|_, _, _| Ok(()));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.toggle_intercept(vcpu, InterceptType::Msr(0x175), true);

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn toggle_msr_intercept_fails_if_monitor_write_ctrlreg_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_monitor_mov_to_msr()
            .returning(|_, _, _| Err(xenctrl::error::XcError::new("some error")));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.toggle_intercept(vcpu, InterceptType::Msr(0x175), true);

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn toggle_singlestep_intercept_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_monitor_singlestep()
            .returning(|_, _| Ok(()));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.toggle_intercept(vcpu, InterceptType::Singlestep, true);

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn toggle_singlestep_intercept_fails_if_monitor_write_ctrlreg_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_monitor_singlestep()
            .returning(|_, _| Err(xenctrl::error::XcError::new("some error")));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.toggle_intercept(vcpu, InterceptType::Singlestep, true);

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    #[test]
    fn toggle_breakpoint_intercept_succeeds() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_monitor_software_breakpoint()
            .returning(|_, _| Ok(()));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.toggle_intercept(vcpu, InterceptType::Breakpoint, true);

        assert!(result.is_ok(), "Expected ok, got error instead");
    }

    #[test]
    fn toggle_breakpoint_intercept_fails_if_monitor_write_ctrlreg_fails() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        setup(
            &mut xencontrol_mock,
            &mut xenevtchn_mock,
            &mut xenforeignmemory_mock,
            &mut xenstore_mock,
        );
        xencontrol_mock
            .expect_monitor_software_breakpoint()
            .returning(|_, _| Err(xenctrl::error::XcError::new("some error")));
        let mut xen = Xen::new(
            "some_vm",
            xencontrol_mock,
            xenevtchn_mock,
            xenforeignmemory_mock,
            xenstore_mock,
            None,
        )
        .expect("Failed to create driver");
        let vcpu = 0;

        let result = xen.toggle_intercept(vcpu, InterceptType::Breakpoint, true);

        assert!(result.is_err(), "Expected error, got ok instead");
    }

    mock! {
        XenControl{}
        trait Debug {
            fn fmt<'a>(&self, f: &mut Formatter<'a>) -> std::fmt::Result;
        }
        trait XenIntrospectable: Debug {
            fn init<'a> (
                &mut self,
                logger: Option<&'a mut xentoollog_logger>,
                dombuild_logger: Option<&'a mut xentoollog_logger>,
                open_flags: u32,
            ) -> Result<(), xenctrl::error::XcError>;
            fn domain_hvm_getcontext_partial(&self, domid: u32, vcpu: u16) -> Result<hvm_hw_cpu, xenctrl::error::XcError>;
            fn domain_hvm_setcontext(&self, domid: u32, buffer: *mut c_uint, size: usize) -> Result<(), xenctrl::error::XcError>;
            fn domain_hvm_getcontext(
                &self,
                domid: u32,
                vcpu: u16,
            ) -> Result<(*mut c_uint, hvm_hw_cpu, u32), xenctrl::error::XcError>;
            fn monitor_enable(&mut self, domid: u32) -> Result<(vm_event_sring, vm_event_back_ring, u32), xenctrl::error::XcError>;
            fn get_request(&self, back_ring: &mut vm_event_back_ring) -> Result<vm_event_request_t, xenctrl::error::XcError>;
            fn put_response(
                &self,
                rsp: &mut vm_event_response_t,
                back_ring: &mut vm_event_back_ring,
            ) -> Result<(), xenctrl::error::XcError>;
            fn get_event_type(&self, req: vm_event_request_t) -> Result<XenEventType, xenctrl::error::XcError>;
            fn monitor_disable(&self, domid: u32) -> Result<(),  xenctrl::error::XcError>;
            fn domain_pause(&self, domid: u32) -> Result<(),  xenctrl::error::XcError>;
            fn domain_unpause(&self, domid: u32) -> Result<(),  xenctrl::error::XcError>;
            fn monitor_software_breakpoint(&self, domid: u32, enable: bool) -> Result<(), xenctrl::error::XcError>;
            fn monitor_singlestep(&self, domid: u32, enable: bool) -> Result<(), xenctrl::error::XcError>;
            fn monitor_mov_to_msr(&self, domid: u32, msr: u32, enable: bool) -> Result<(), xenctrl::error::XcError>;
            fn monitor_write_ctrlreg(
                &self,
                domid: u32,
                index: XenCr,
                enable: bool,
                sync: bool,
                onchangeonly: bool,
            ) -> Result<(), xenctrl::error::XcError>;
            fn set_mem_access(
                &self,
                domid: u32,
                access: XenPageAccess,
                first_pfn: u64,
            ) -> Result<(), xenctrl::error::XcError>;
            fn get_mem_access(&self, domid: u32, pfn: u64) -> Result<XenPageAccess, xenctrl::error::XcError>;
            fn domain_maximum_gpfn(&self, domid: u32) -> Result<u64, xenctrl::error::XcError>;
            fn close(&mut self) -> Result<(), xenctrl::error::XcError>;
        }
    }

    mock! {
            XenEventChannel{}
            trait Debug {
                fn fmt<'a>(&self, f: &mut Formatter<'a>) -> std::fmt::Result;
            }
            trait  EventChannelSetup: Debug {
                fn init(&mut self, domid: u32, evtchn_port: u32) -> Result<(), std::io::Error>;
                fn get_bind_port(&self) -> i32;
                fn xenevtchn_pending(&self) -> Result<xenevtchn_port_or_error_t, std::io::Error>;
                fn xenevtchn_fd(&self) -> Result<i32, std::io::Error>;
                fn xenevtchn_unmask(&self, port: evtchn_port_t) -> Result<(), std::io::Error>;
                fn xenevtchn_notify(&self) -> Result<(), std::io::Error>;
        }
    }

    mock! {
        Xs{}
        trait Debug {
            fn fmt<'a>(&self, f: &mut Formatter<'a>) -> std::fmt::Result;
        }
        trait  XsIntrospectable: Debug {
            fn init(&mut self, open_type: XsOpenFlags) -> Result<(), std::io::Error>;
            fn directory(&self, transaction: XBTransaction, path: String) -> Vec<String>;
            fn read(&self, transaction: XBTransaction, path: String) -> String;
            fn close(&mut self);
        }
    }

    mock! {
        XenForeignMem{}
        trait Debug {
            fn fmt<'a>(&self, f: &mut Formatter<'a>) -> std::fmt::Result;
        }
        trait  XenForeignMemoryIntrospectable: Debug {
            fn init(&mut self) -> Result<(), failure::Error>;
            fn map<'a> (&'a self, domid: u32, prot: c_int, gfn: u64) -> Result<&'a mut [u8], failure::Error>;
            fn unmap(&self, page: &mut [u8]) -> Result<(), Box<std::io::Error>>;
            fn close(&mut self) -> Result<(), failure::Error>;
        }
    }
}
