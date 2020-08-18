use crate::api::{
    Access, CrType, DriverInitParam, Event, EventType, InterceptType, Introspectable, Registers,
    SegmentReg, X86Registers,
};
use std::convert::{From, TryFrom};
use std::error::Error;
use std::mem;

use libc::PROT_READ;
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
    use mockall::predicate::{eq, function};
    use std::fmt::{Debug, Formatter};
    use test_case::test_case;
    use xenctrl::{
        hvm_hw_cpu, vm_event_back_ring, vm_event_request_t, vm_event_response_t, vm_event_sring,
        xentoollog_logger, XenEventType, XenPageAccess,
    };
    use xenevtchn::{evtchn_port_t, xenevtchn_port_or_error_t};
    use xenforeignmemory::XenForeignMem;
    use xenstore::{XBTransaction, XsOpenFlags};
    use std::os::raw::{c_uint, c_int};

    #[test]
    fn test_fail_to_create_kvm_driver_if_xencontrol_init_returns_error() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        xencontrol_mock.expect_init().returning(|_, _, _| {
            Err(xenctrl::error::XcError::new())
        });
        xenevtchn_mock.expect_init().returning(|_, _| Ok(()));
        xenforeignmemory_mock.expect_init().returning(|| Ok(()));
        xenstore_mock.expect_init().returning(|_| Ok(()));

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
    fn test_fail_to_create_kvm_driver_if_xenevtchn_init_returns_error() {
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
    fn test_fail_to_create_kvm_driver_if_xenforeignmemory_init_returns_error() {
        let mut xencontrol_mock = MockXenControl::default();
        let mut xenevtchn_mock = MockXenEventChannel::default();
        let mut xenforeignmemory_mock = MockXenForeignMem::default();
        let mut xenstore_mock = MockXs::default();
        xencontrol_mock.expect_init().returning(|_, _, _| Ok(()));
        xenevtchn_mock.expect_init().returning(|_, _| Ok(()));
        xenforeignmemory_mock.expect_init().returning(|| {
            Err(failure::Error::new())
        });
        xenstore_mock.expect_init().returning(|_| Ok(()));

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
    fn test_fail_to_create_kvm_driver_if_xenstore_init_returns_error() {
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

    mock! {
        XenControl{}
        trait Debug {
            fn fmt<'a>(&self, f: &mut Formatter<'a>) -> std::fmt::Result;
        }
        trait XenIntrospectable: Debug {
            fn init(
                &mut self,
                logger: Option<&mut xentoollog_logger>,
                dombuild_logger: Option<&mut xentoollog_logger>,
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
            fn map(&self, domid: u32, prot: c_int, gfn: u64) -> Result<&mut [u8], failure::Error>;
            fn unmap(&self, page: &mut [u8]) -> Result<(), Box<std::io::Error>>;
            fn close(&mut self) -> Result<(), failure::Error>;
        }
    }
}
