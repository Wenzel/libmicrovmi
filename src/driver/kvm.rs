use kvmi::{
    kvm_dtable, kvm_regs, kvm_segment, KVMIntrospectable, KVMiCr, KVMiEvent, KVMiEventReply,
    KVMiEventType, KVMiInterceptType, KVMiPageAccess,
};
use std::convert::From;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::error::Error;
use std::mem;
use std::vec::Vec;

use crate::api::{
    Access, CrType, DriverInitParam, Event, EventReplyType, EventType, InterceptType,
    Introspectable, Registers, SegmentReg, SystemTableReg, X86Registers, PAGE_SHIFT,
};

impl TryFrom<Access> for KVMiPageAccess {
    type Error = &'static str;
    fn try_from(access: Access) -> Result<Self, Self::Error> {
        match access {
            Access::NIL => Ok(KVMiPageAccess::NIL),
            Access::R => Ok(KVMiPageAccess::R),
            Access::W => Ok(KVMiPageAccess::W),
            Access::RW => Ok(KVMiPageAccess::RW),
            Access::X => Ok(KVMiPageAccess::X),
            Access::RX => Ok(KVMiPageAccess::RX),
            Access::WX => Ok(KVMiPageAccess::WX),
            Access::RWX => Ok(KVMiPageAccess::RWX),
            _ => Err("invalid access value"),
        }
    }
}

impl From<KVMiPageAccess> for Access {
    fn from(access: KVMiPageAccess) -> Self {
        match access {
            KVMiPageAccess::NIL => Access::NIL,
            KVMiPageAccess::R => Access::R,
            KVMiPageAccess::W => Access::W,
            KVMiPageAccess::RW => Access::RW,
            KVMiPageAccess::X => Access::X,
            KVMiPageAccess::RX => Access::RX,
            KVMiPageAccess::WX => Access::WX,
            KVMiPageAccess::RWX => Access::RWX,
        }
    }
}

impl From<kvm_segment> for SegmentReg {
    fn from(segment: kvm_segment) -> Self {
        SegmentReg {
            base: segment.base,
            limit: segment.limit,
            selector: segment.selector,
        }
    }
}

impl From<kvm_dtable> for SystemTableReg {
    fn from(dtable: kvm_dtable) -> Self {
        SystemTableReg {
            base: dtable.base,
            limit: dtable.limit,
        }
    }
}

impl From<X86Registers> for kvm_regs {
    fn from(register: X86Registers) -> Self {
        kvm_regs {
            rax: register.rax,
            rbx: register.rbx,
            rcx: register.rcx,
            rdx: register.rdx,
            rsi: register.rsi,
            rdi: register.rdi,
            rsp: register.rsp,
            rbp: register.rbp,
            r8: register.r8,
            r9: register.r9,
            r10: register.r10,
            r11: register.r11,
            r12: register.r12,
            r13: register.r13,
            r14: register.r14,
            r15: register.r15,
            rip: register.rip,
            rflags: register.rflags,
        }
    }
}

#[derive(Debug)]
pub struct Kvm<T: KVMIntrospectable> {
    kvmi: T,
    expect_pause_ev: u32,
    // VCPU -> KVMiEvent
    vec_events: Vec<Option<KVMiEvent>>,
}

impl<T: KVMIntrospectable> Kvm<T> {
    pub fn new(
        domain_name: &str,
        mut kvmi: T,
        init_option: Option<DriverInitParam>,
    ) -> Result<Self, Box<dyn Error>> {
        let DriverInitParam::KVMiSocket(socket_path) = init_option
            .expect("KVM driver initialization requires an additional socket parameter.");
        debug!("init on {} (socket: {})", domain_name, socket_path);
        kvmi.init(&socket_path)?;
        let mut kvm = Kvm {
            kvmi,
            expect_pause_ev: 0,
            vec_events: Vec::new(),
        };

        // set vec_events size
        let vcpu_count = kvm.get_vcpu_count().unwrap();
        kvm.vec_events
            .resize_with(vcpu_count.try_into().unwrap(), || None);

        // enable CR event intercept by default
        // (interception will take place when CR register will be specified)
        for vcpu in 0..vcpu_count {
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Cr, true)
                .unwrap();
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Msr, true)
                .unwrap();
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Pagefault, true)
                .unwrap();
        }

        Ok(kvm)
    }
}

impl<T: KVMIntrospectable> Introspectable for Kvm<T> {
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        Ok(self.kvmi.get_vcpu_count().unwrap().try_into()?)
    }

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        Ok(self.kvmi.read_physical(paddr, buf)?)
    }

    fn write_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        Ok(self.kvmi.write_physical(paddr, buf)?)
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        let max_gfn = self.kvmi.get_maximum_gfn()?;
        Ok(max_gfn << PAGE_SHIFT)
    }

    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let (regs, sregs, msrs) = self.kvmi.get_registers(vcpu)?;
        let msrs_as_slice = msrs.as_slice();
        // TODO: hardcoded for x86 for now
        Ok(Registers::X86(X86Registers {
            rax: regs.rax,
            rbx: regs.rbx,
            rcx: regs.rcx,
            rdx: regs.rdx,
            rsi: regs.rsi,
            rdi: regs.rdi,
            rsp: regs.rsp,
            rbp: regs.rbp,
            r8: regs.r8,
            r9: regs.r9,
            r10: regs.r10,
            r11: regs.r11,
            r12: regs.r12,
            r13: regs.r13,
            r14: regs.r14,
            r15: regs.r15,
            rip: regs.rip,
            rflags: regs.rflags,
            cr0: sregs.cr0,
            cr2: sregs.cr2,
            cr3: sregs.cr3,
            cr4: sregs.cr4,
            sysenter_cs: msrs_as_slice[0].data,
            sysenter_esp: msrs_as_slice[1].data,
            sysenter_eip: msrs_as_slice[2].data,
            msr_efer: msrs_as_slice[3].data,
            msr_star: msrs_as_slice[4].data,
            msr_lstar: msrs_as_slice[5].data,
            efer: sregs.efer,
            apic_base: sregs.apic_base,
            cs: sregs.cs.into(),
            ds: sregs.ds.into(),
            es: sregs.es.into(),
            fs: sregs.fs.into(),
            gs: sregs.gs.into(),
            ss: sregs.ss.into(),
            tr: sregs.tr.into(),
            ldt: sregs.ldt.into(),
            idt: sregs.idt.into(),
            gdt: sregs.gdt.into(),
        }))
    }

    fn write_registers(&self, vcpu: u16, reg: Registers) -> Result<(), Box<dyn Error>> {
        match reg {
            Registers::X86(x86_registers) => {
                self.kvmi.set_registers(vcpu, &x86_registers.into())?;
            }
        }
        Ok(())
    }

    fn get_page_access(&self, paddr: u64) -> Result<Access, Box<dyn Error>> {
        let access = self.kvmi.get_page_access(paddr)?;
        Ok(access.try_into().unwrap())
    }

    fn set_page_access(&self, paddr: u64, access: Access) -> Result<(), Box<dyn Error>> {
        self.kvmi
            .set_page_access(paddr, access.try_into().unwrap())?;
        Ok(())
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("pause");
        // already paused ?
        if self.expect_pause_ev > 0 {
            return Ok(());
        }

        self.kvmi.pause()?;
        self.expect_pause_ev = self.kvmi.get_vcpu_count()?;
        debug!("expected pause events: {}", self.expect_pause_ev);
        Ok(())
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("resume");
        // already resumed ?
        if self.expect_pause_ev == 0 {
            return Ok(());
        }

        while self.expect_pause_ev > 0 {
            // wait
            let kvmi_event = self.kvmi.wait_and_pop_event(1000)?.unwrap();
            match kvmi_event.ev_type {
                KVMiEventType::PauseVCPU => {
                    debug!("VCPU {} - Received Pause Event", kvmi_event.vcpu);
                    self.expect_pause_ev -= 1;
                    self.kvmi.reply(&kvmi_event, KVMiEventReply::Continue)?;
                }
                _ => panic!(
                    "Unexpected {:?} event type while resuming VM",
                    kvmi_event.ev_type
                ),
            }
        }
        Ok(())
    }

    fn toggle_intercept(
        &mut self,
        vcpu: u16,
        intercept_type: InterceptType,
        enabled: bool,
    ) -> Result<(), Box<dyn Error>> {
        match intercept_type {
            InterceptType::Cr(micro_cr_type) => {
                let kvmi_cr = match micro_cr_type {
                    CrType::Cr0 => KVMiCr::Cr0,
                    CrType::Cr3 => KVMiCr::Cr3,
                    CrType::Cr4 => KVMiCr::Cr4,
                };
                Ok(self.kvmi.control_cr(vcpu, kvmi_cr, enabled)?)
            }
            InterceptType::Msr(micro_msr_type) => {
                Ok(self.kvmi.control_msr(vcpu, micro_msr_type, enabled)?)
            }
            InterceptType::Breakpoint => {
                Ok(self
                    .kvmi
                    .control_events(vcpu, KVMiInterceptType::Breakpoint, enabled)?)
            }
            InterceptType::Pagefault => {
                Ok(self
                    .kvmi
                    .control_events(vcpu, KVMiInterceptType::Pagefault, enabled)?)
            }
        }
    }

    fn listen(&mut self, timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        // wait for next event and pop it
        debug!("wait for next event");
        let kvmi_event_opt = self.kvmi.wait_and_pop_event(timeout.try_into().unwrap())?;
        match kvmi_event_opt {
            None => Ok(None),
            Some(kvmi_event) => {
                let microvmi_event_kind = match kvmi_event.ev_type {
                    KVMiEventType::Cr { cr_type, new, old } => EventType::Cr {
                        cr_type: match cr_type {
                            KVMiCr::Cr0 => CrType::Cr0,
                            KVMiCr::Cr3 => CrType::Cr3,
                            KVMiCr::Cr4 => CrType::Cr4,
                        },
                        new,
                        old,
                    },
                    KVMiEventType::Msr { msr_type, new, old } => EventType::Msr {
                        msr_type,
                        new,
                        old,
                    },
                    KVMiEventType::Breakpoint {gpa, insn_len } =>  EventType::Breakpoint {
                        gpa,
                        insn_len,
                    },
                    KVMiEventType::Pagefault {gva, gpa, access, view: _} =>  EventType::Pagefault {
                        gva,
                        gpa,
                        access: access.into(),
                    },
                    KVMiEventType::PauseVCPU => panic!("Unexpected PauseVCPU event. It should have been popped by resume VM. (Did you forget to resume your VM ?)"),
                };

                let vcpu = kvmi_event.vcpu;
                let vcpu_index: usize = vcpu.try_into().unwrap();
                self.vec_events[vcpu_index] = Some(kvmi_event);

                Ok(Some(Event {
                    vcpu,
                    kind: microvmi_event_kind,
                }))
            }
        }
    }

    fn reply_event(
        &mut self,
        event: Event,
        reply_type: EventReplyType,
    ) -> Result<(), Box<dyn Error>> {
        let kvm_reply_type = match reply_type {
            EventReplyType::Continue => KVMiEventReply::Continue,
        };
        // get KVMiEvent associated with this VCPU
        let vcpu_index: usize = event.vcpu.try_into().unwrap();
        let kvmi_event = mem::replace(&mut self.vec_events[vcpu_index], None).unwrap();
        Ok(self.kvmi.reply(&kvmi_event, kvm_reply_type)?)
    }
}

impl<T: KVMIntrospectable> Drop for Kvm<T> {
    fn drop(&mut self) {
        debug!("KVM driver close");
        // disable all control register interception
        for vcpu in 0..self.get_vcpu_count().unwrap() {
            self.kvmi
                .control_events(vcpu, KVMiInterceptType::Cr, false)
                .unwrap();
            self.kvmi
                .control_events(vcpu, KVMiInterceptType::Msr, false)
                .unwrap();
            self.kvmi
                .control_events(vcpu, KVMiInterceptType::Pagefault, false)
                .unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvmi::{kvm_regs, kvm_sregs, KVMiCr, KvmMsrs};
    use mockall::mock;
    use mockall::predicate::{eq, function};
    use std::fmt::{Debug, Formatter};
    use test_case::test_case;

    #[test_case(1; "single vcpu")]
    #[test_case(2; "two vcpus")]
    #[test_case(16; "sixteen vcpus")]
    fn test_create_kvm_driver_if_guest_domain_is_valid(vcpu_count: u32) {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock.expect_init().returning(|_| Ok(()));
        kvmi_mock
            .expect_get_vcpu_count()
            .returning(move || Ok(vcpu_count));
        for vcpu in 0..vcpu_count {
            kvmi_mock
                .expect_control_events()
                .with(
                    eq(vcpu as u16),
                    function(|x| matches!(x, KVMiInterceptType::Cr)),
                    eq(true),
                )
                .times(1)
                .returning(|_, _, _| Ok(()));
            kvmi_mock
                .expect_control_events()
                .with(
                    eq(vcpu as u16),
                    function(|x| matches!(x, KVMiInterceptType::Cr)),
                    eq(false),
                )
                .times(1)
                .returning(|_, _, _| Ok(()));
            kvmi_mock
                .expect_control_events()
                .with(
                    eq(vcpu as u16),
                    function(|x| matches!(x, KVMiInterceptType::Msr)),
                    eq(true),
                )
                .times(1)
                .returning(|_, _, _| Ok(()));
            kvmi_mock
                .expect_control_events()
                .with(
                    eq(vcpu as u16),
                    function(|x| matches!(x, KVMiInterceptType::Msr)),
                    eq(false),
                )
                .times(1)
                .returning(|_, _, _| Ok(()));
            kvmi_mock
                .expect_control_events()
                .with(
                    eq(vcpu as u16),
                    function(|x| matches!(x, KVMiInterceptType::Pagefault)),
                    eq(true),
                )
                .times(1)
                .returning(|_, _, _| Ok(()));
            kvmi_mock
                .expect_control_events()
                .with(
                    eq(vcpu as u16),
                    function(|x| matches!(x, KVMiInterceptType::Pagefault)),
                    eq(false),
                )
                .times(1)
                .returning(|_, _, _| Ok(()));
        }

        let result = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        );

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_fail_to_create_kvm_driver_if_kvmi_init_returns_error() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock.expect_init().returning(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });

        let result = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        );

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_pause_vcpu_when_vm_not_paused() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_pause().times(1).returning(|| Ok(()));
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let result = kvm.pause();

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_pause_vcpu_when_vm_is_paused() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_pause().times(1).returning(|| Ok(()));
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let _result_pause_happens = kvm.pause();
        let result_pause_does_not_happen = kvm.pause();

        assert!(
            result_pause_does_not_happen.is_ok(),
            "Expected ok, got error instead!"
        );
    }

    #[test]
    fn test_pause_vcpu_fails_when_kvmi_pause_fails() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock.expect_pause().times(1).returning(|| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let result = kvm.pause();

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_listen_events_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        let cr_type = KVMiCr::Cr3;
        let new = 0;
        let old = 0;
        kvmi_mock
            .expect_wait_and_pop_event()
            .times(1)
            .returning(move |_| {
                Ok(Some(KVMiEvent {
                    vcpu: 0,
                    ev_type: KVMiEventType::Cr { cr_type, new, old },
                    ffi_event: std::ptr::null_mut(),
                }))
            });
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let timeout = 0;
        let cr_type_microvmi = CrType::Cr3;
        let result = kvm.listen(timeout).expect("Failed to listen for events");
        let event = result.unwrap();

        assert_eq!(
            EventType::Cr {
                cr_type: cr_type_microvmi,
                new,
                old
            },
            event.kind
        );
    }

    #[test]
    fn test_listen_events_fails_if_kvmi_wait_and_pop_fails() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock
            .expect_wait_and_pop_event()
            .times(1)
            .returning(|_| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "something went wrong",
                ))
            });
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let timeout = 0;
        let result = kvm.listen(timeout);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_set_page_access_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock
            .expect_set_page_access()
            .times(1)
            .with(eq(0), eq(KVMiPageAccess::R))
            .returning(|_, _| Ok(()));
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let paddr: u64 = 0;
        let access = Access::R;
        let result = kvm.set_page_access(paddr, access);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_set_page_access_fails_if_kvmi_set_page_access_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock
            .expect_set_page_access()
            .times(1)
            .returning(|_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "something went wrong",
                ))
            });
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let paddr: u64 = 0;
        let access = Access::R;
        let result = kvm.set_page_access(paddr, access);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_get_page_access_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        let access = KVMiPageAccess::R;
        kvmi_mock
            .expect_get_page_access()
            .times(1)
            .returning(move |_| Ok(access));
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let paddr: u64 = 0;
        let permission = kvm.get_page_access(paddr).unwrap();

        assert_eq!(Access::R, permission);
    }

    #[test]
    fn test_get_page_access_fails_if_kvmi_get_page_access_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_get_page_access().times(1).returning(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let paddr: u64 = 0;
        let result = kvm.get_page_access(paddr);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_get_max_physical_addr_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock
            .expect_get_maximum_gfn()
            .times(1)
            .returning(move || Ok(0));
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let addr = kvm.get_max_physical_addr().unwrap();

        assert_eq!(0, addr);
    }

    #[test]
    fn test_get_max_physical_addr_fails_if_kvmi_get_maximum_gfn_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_get_maximum_gfn().times(1).returning(|| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let result = kvm.get_max_physical_addr();

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_resume_vcpu_with_multiple_pause_events() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_pause().times(1).returning(|| Ok(()));
        kvmi_mock
            .expect_wait_and_pop_event()
            .times(1)
            .returning(|_| {
                Ok(Some(KVMiEvent {
                    vcpu: 0,
                    ev_type: KVMiEventType::PauseVCPU,
                    ffi_event: std::ptr::null_mut(),
                }))
            });
        kvmi_mock.expect_reply().times(1).returning(|_, _| Ok(()));
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let _result_pause = kvm.pause();
        let result_resume = kvm.resume();

        assert!(result_resume.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_resume_vcpu_with_no_pause_events() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock
            .expect_wait_and_pop_event()
            .times(0)
            .returning(|_| {
                Ok(Some(KVMiEvent {
                    vcpu: 0,
                    ev_type: KVMiEventType::PauseVCPU,
                    ffi_event: std::ptr::null_mut(),
                }))
            });
        kvmi_mock.expect_reply().times(0).returning(|_, _| Ok(()));
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let result = kvm.resume();

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_resume_vcpu_fails_if_kvmi_wait_and_pop_event_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_pause().times(1).returning(|| Ok(()));
        kvmi_mock
            .expect_wait_and_pop_event()
            .times(1)
            .returning(|_| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "something went wrong",
                ))
            });
        kvmi_mock.expect_reply().times(0).returning(|_, _| Ok(()));
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let _result_pause = kvm.pause();
        let result = kvm.resume();

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_resume_vcpu_fails_if_kvmi_reply_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_pause().times(1).returning(|| Ok(()));
        kvmi_mock
            .expect_wait_and_pop_event()
            .times(1)
            .returning(|_| {
                Ok(Some(KVMiEvent {
                    vcpu: 0,
                    ev_type: KVMiEventType::PauseVCPU,
                    ffi_event: std::ptr::null_mut(),
                }))
            });
        kvmi_mock.expect_reply().times(1).returning(|_, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let _result_pause = kvm.pause();
        let result = kvm.resume();

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_read_physical_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        let mut buffer: [u8; 4096] = [0; 4096];
        kvmi_mock
            .expect_read_physical()
            .times(1)
            .returning(|_, buffer| {
                buffer[0] = 1;
                Ok(())
            });
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let paddr: u64 = 0;

        let _result = kvm.read_physical(paddr, &mut buffer);

        assert_eq!(1, buffer[0]);
    }

    #[test]
    fn test_read_physical_fails_if_kvmi_read_physical_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_read_physical().times(1).returning(|_, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let paddr: u64 = 0;
        let mut buffer: [u8; 4096] = [0; 4096];

        let result = kvm.read_physical(paddr, &mut buffer);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_write_physical_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        let paddr: u64 = 0;
        let mut buffer: [u8; 16] = [1; 16];
        kvmi_mock
            .expect_write_physical()
            .times(1)
            .with(eq(paddr), function(move |x: &[u8]| x == buffer))
            .returning(|_, _| Ok(()));
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");

        let result = kvm.write_physical(paddr, &mut buffer);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_write_physical_fails_if_kvmi_write_physical_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock
            .expect_write_physical()
            .times(1)
            .returning(|_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "something went wrong",
                ))
            });
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let paddr: u64 = 0;
        let mut buffer: [u8; 4096] = [1; 4096];

        let result = kvm.write_physical(paddr, &mut buffer);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_write_registers_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        let x86_register_struct: X86Registers = Default::default();
        kvmi_mock
            .expect_set_registers()
            .times(1)
            .returning(|_, _| Ok(()));
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;

        let result = kvm.write_registers(vcpu, Registers::X86(x86_register_struct));

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_write_registers_fails_if_kvmi_set_registers_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        let x86_register_struct: X86Registers = Default::default();
        kvmi_mock.expect_set_registers().times(1).returning(|_, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;

        let result = kvm.write_registers(vcpu, Registers::X86(x86_register_struct));

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_read_registers_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        let regs: kvm_regs = Default::default();
        let sregs: kvm_sregs = Default::default();
        let msrs = KvmMsrs::new();
        kvmi_mock
            .expect_get_registers()
            .times(1)
            .return_once(move |_| Ok((regs, sregs, msrs)));
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;

        let result = kvm.read_registers(vcpu);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_read_registers_fails_if_kvmi_get_registers_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock.expect_get_registers().times(1).returning(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
        let kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;

        let result = kvm.read_registers(vcpu);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_cr_intercept_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock
            .expect_control_cr()
            .with(eq(0), function(|x| matches!(x, KVMiCr::Cr3)), eq(true))
            .times(1)
            .returning(|_, _, _| Ok(()));
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;
        let intercept_type = InterceptType::Cr(CrType::Cr3);
        let enabled: bool = true;

        let result = kvm.toggle_intercept(vcpu, intercept_type, enabled);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_cr_intercept_fails_if_kvmi_control_cr_fails() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock
            .expect_control_cr()
            .times(1)
            .with(eq(0), function(|x| matches!(x, KVMiCr::Cr3)), eq(true))
            .returning(|_, _, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "something went wrong",
                ))
            });
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;
        let intercept_type = InterceptType::Cr(CrType::Cr3);
        let enabled: bool = true;

        let result = kvm.toggle_intercept(vcpu, intercept_type, enabled);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_msr_intercept_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock
            .expect_control_msr()
            .times(1)
            .with(eq(0), eq(0x175 as u32), eq(true))
            .returning(|_, _, _| Ok(()));
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;
        let intercept_type = InterceptType::Msr(0x175 as u32);
        let enabled: bool = true;

        let result = kvm.toggle_intercept(vcpu, intercept_type, enabled);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_msr_intercept_fails_if_kvmi_control_msr_fails() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock
            .expect_control_msr()
            .times(1)
            .with(eq(0), eq(0x175 as u32), eq(true))
            .returning(|_, _, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "something went wrong",
                ))
            });
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;
        let intercept_type = InterceptType::Msr(0x175 as u32);
        let enabled: bool = true;

        let result = kvm.toggle_intercept(vcpu, intercept_type, enabled);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_breakpoint_intercept_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Breakpoint)),
                eq(true),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;
        let intercept_type = InterceptType::Breakpoint;
        let enabled = true;

        let result = kvm.toggle_intercept(vcpu, intercept_type, enabled);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_breakpoint_intercept_fails_if_kvmi_conrol_events_fails() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Breakpoint)),
                eq(true),
            )
            .times(1)
            .returning(|_, _, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "something went wrong",
                ))
            });
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;
        let intercept_type = InterceptType::Breakpoint;
        let enabled = true;

        let result = kvm.toggle_intercept(vcpu, intercept_type, enabled);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    #[test]
    fn test_pf_intercept_succeeds() {
        let mut kvmi_mock = MockKVMi::default();
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Pagefault)),
                eq(true),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));
        setup_kvmi_mock(&mut kvmi_mock);
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;
        let intercept_type = InterceptType::Pagefault;
        let enabled = true;

        let result = kvm.toggle_intercept(vcpu, intercept_type, enabled);

        assert!(result.is_ok(), "Expected ok, got error instead!");
    }

    #[test]
    fn test_pf_intercept_fails_if_kvmi_conrol_events_fails() {
        let mut kvmi_mock = MockKVMi::default();
        setup_kvmi_mock(&mut kvmi_mock);
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Pagefault)),
                eq(true),
            )
            .times(1)
            .returning(|_, _, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "something went wrong",
                ))
            });
        let mut kvm = Kvm::new(
            "some_vm",
            kvmi_mock,
            Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())),
        )
        .expect("Failed to create driver");
        let vcpu: u16 = 0;
        let intercept_type = InterceptType::Pagefault;
        let enabled = true;

        let result = kvm.toggle_intercept(vcpu, intercept_type, enabled);

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    fn setup_kvmi_mock(kvmi_mock: &mut MockKVMi) {
        kvmi_mock.expect_init().times(1).returning(|_| Ok(()));
        kvmi_mock.expect_get_vcpu_count().returning(move || Ok(1));
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Cr)),
                eq(true),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Cr)),
                eq(false),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Msr)),
                eq(true),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Msr)),
                eq(false),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Pagefault)),
                eq(true),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));
        kvmi_mock
            .expect_control_events()
            .with(
                eq(0),
                function(|x| matches!(x, KVMiInterceptType::Pagefault)),
                eq(false),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));
    }

    mock! {
        KVMi{}
        trait Debug {
            fn fmt<'a>(&self, f: &mut Formatter<'a>) -> std::fmt::Result;
        }
        trait KVMIntrospectable: Debug {
            fn init(&mut self, socket_path: &str) -> Result<(), std::io::Error>;
            fn control_events(
                &self,
                vcpu: u16,
                intercept_type: KVMiInterceptType,
                enabled: bool,
            ) -> Result<(), std::io::Error>;
            fn control_cr(&self, vcpu: u16, reg: KVMiCr, enabled: bool) -> Result<(), std::io::Error>;
            fn control_msr(&self, vcpu: u16, reg: u32, enabled: bool) -> Result<(), std::io::Error>;
            fn read_physical(&self, gpa: u64, buffer: &mut [u8]) -> Result<(), std::io::Error>;
            fn write_physical(&self, gpa: u64, buffer: &[u8]) -> Result<(), std::io::Error>;
            fn get_page_access(&self, gpa: u64) -> Result<KVMiPageAccess, std::io::Error>;
            fn set_page_access(&self, gpa: u64, access: KVMiPageAccess) -> Result<(), std::io::Error>;
            fn pause(&self) -> Result<(), std::io::Error>;
            fn get_vcpu_count(&self) -> Result<u32, std::io::Error>;
            fn get_registers(&self, vcpu: u16) -> Result<(kvm_regs, kvm_sregs, KvmMsrs), std::io::Error>;
            fn set_registers(&self, vcpu: u16, regs: &kvm_regs) -> Result<(), std::io::Error>;
            fn wait_and_pop_event(&self, ms: i32) -> Result<Option<KVMiEvent>, std::io::Error>;
            fn reply(&self, event: &KVMiEvent, reply_type: KVMiEventReply) -> Result<(), std::io::Error>;
            fn get_maximum_gfn(&self) -> Result<u64, std::io::Error>;
        }
    }
}
