#[cfg(test)]
use mockall::automock;
use std::convert::TryInto;
use std::error::Error;
use std::mem;
use std::vec::Vec;

use kvmi::{
    kvmi_sys::kvm_msrs, kvmi_sys::kvm_regs, kvmi_sys::kvm_sregs, KVMi, KVMiCr, KVMiEvent,
    KVMiEventReply, KVMiEventType, KVMiInterceptType, KVMiPageAccess, KVMiRegs, KvmMsr,
};

//use kvmi_sys::*;

use crate::api::*;

pub struct Kvm {
    kvmi: Box<dyn KVMIntrospectable>,
    expect_pause_ev: u32,
    // VCPU -> KVMiEvent
    vec_events: Vec<Option<KVMiEvent>>,
}

impl Kvm {
    pub fn new(
        domain_name: &str,
        mut kvmi: Box<dyn KVMIntrospectable>,
    ) -> Result<Self, Box<dyn Error>> {
        let socket_path = "/tmp/introspector";
        debug!("init on {} (socket: {})", domain_name, socket_path);
        kvmi.init(socket_path)?;
        let mut kvm = Kvm {
            kvmi,
            expect_pause_ev: 0,
            vec_events: Vec::new(),
        };

        // set vec_events size
        let vcpu_count = kvm.get_vcpu_count().unwrap();
        kvm.vec_events
            .resize_with(vcpu_count.try_into().unwrap(), || None);

        // Enable intercepts for all the vcpus
        for vcpu in 0..vcpu_count {
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Cr, true)
                .unwrap();
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Msr, true)
                .unwrap();
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Breakpoint, true)
                .unwrap();
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Pagefault, true)
                .unwrap();
        }

        Ok(kvm)
    }
}

impl Introspectable for Kvm {
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        Ok(self.kvmi.get_vcpu_count().unwrap().try_into()?)
    }

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        Ok(self.kvmi.read_physical(paddr, buf)?)
    }

    fn write_physical(&self, paddr: u64, buf: &[u8]) -> Result<(), Box<dyn Error>> {
        Ok(self.kvmi.write_physical(paddr, buf)?)
    }

    fn get_page_access(&self, paddr: u64) -> Result<u8, Box<dyn Error>> {
        Ok(self.kvmi.get_page_access(paddr).unwrap().try_into()?)
    }

    fn set_page_access(&self, paddr: u64, access: u8) -> Result<(), Box<dyn Error>> {
        Ok(self
            .kvmi
            .set_page_access(paddr, access)
            .unwrap()
            .try_into()?)
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        let max_gfn = self.kvmi.get_maximum_gfn()?;
        Ok(max_gfn << PAGE_SHIFT)
    }

    fn handle_pf_event(&self, paddr: u64, pf_access: u8) -> Result<(), Box<dyn Error>> {
        let read_permission = KVMiPageAccess::PageAccessR as u8;
        let write_permission = KVMiPageAccess::PageAccessW as u8;
        let execute_permission = KVMiPageAccess::PageAccessX as u8;
        let mut access: u8;
        if pf_access & read_permission != 0 {
            access = self.kvmi.get_page_access(paddr)?;
            access |= read_permission;
            self.kvmi.set_page_access(paddr, access)?;
        }
        if pf_access & write_permission != 0 {
            access = self.kvmi.get_page_access(paddr)?;
            access |= write_permission;
            self.kvmi.set_page_access(paddr, access)?;
        }
        if pf_access & execute_permission != 0 {
            access = self.kvmi.get_page_access(paddr)?;
            access |= execute_permission;
            self.kvmi.set_page_access(paddr, access)?;
        }
        Ok(())
    }

    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let (regs, sregs, msrs) = self.kvmi.get_registers(vcpu)?;
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
            sysenter_cs: msrs.entries[0].data,
            sysenter_esp: msrs.entries[1].data,
            sysenter_eip: msrs.entries[2].data,
            msr_efer: msrs.entries[3].data,
            msr_star: msrs.entries[4].data,
            msr_lstar: msrs.entries[5].data,
            efer: sregs.efer,
            apic_base: sregs.apic_base,
            cs: segment_reg {
                base: sregs.cs.base,
                limit: sregs.cs.limit,
                selector: sregs.cs.selector,
            },
            ds: segment_reg {
                base: sregs.ds.base,
                limit: sregs.ds.limit,
                selector: sregs.ds.selector,
            },
            es: segment_reg {
                base: sregs.es.base,
                limit: sregs.es.limit,
                selector: sregs.es.selector,
            },
            fs: segment_reg {
                base: sregs.fs.base,
                limit: sregs.fs.limit,
                selector: sregs.fs.selector,
            },
            gs: segment_reg {
                base: sregs.gs.base,
                limit: sregs.gs.limit,
                selector: sregs.gs.selector,
            },
            ss: segment_reg {
                base: sregs.ss.base,
                limit: sregs.ss.limit,
                selector: sregs.ss.selector,
            },
            tr: segment_reg {
                base: sregs.tr.base,
                limit: sregs.tr.limit,
                selector: sregs.tr.selector,
            },
            ldt: segment_reg {
                base: sregs.ldt.base,
                limit: sregs.ldt.limit,
                selector: sregs.ldt.selector,
            },
        }))
    }

    fn write_registers(&self, vcpu: u16, reg: Registers) -> Result<(), Box<dyn Error>> {
        let register: X86Registers;
        match reg {
            Registers::X86(x86_registers) => {
                register = x86_registers;
            }
        }
        let regs = KVMiRegs {
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
        };
        //Set the value of the register by calling set_registers() function of the kvmi crate.
        self.kvmi.set_registers(vcpu, &regs)?;
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
            // pop vcpu pause events
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
        // Call appropriate functions to handle various types of events
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
                    KVMiEventType::Pagefault {gva, gpa, access, view} =>  EventType::Pagefault {
                        gva,
                        gpa,
                        access,
                        view,
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

    fn get_driver_type(&self) -> DriverType {
        DriverType::KVM
    }
}

impl Drop for Kvm {
    fn drop(&mut self) {
        debug!("KVM driver close");
        // disable all event intercepts.
        for vcpu in 0..self.get_vcpu_count().unwrap() {
            self.kvmi
                .control_events(vcpu, KVMiInterceptType::Cr, false)
                .unwrap();
            self.kvmi
                .control_events(vcpu, KVMiInterceptType::Msr, false)
                .unwrap();
            self.kvmi
                .control_events(vcpu, KVMiInterceptType::Breakpoint, false)
                .unwrap();
            self.kvmi
                .control_events(vcpu, KVMiInterceptType::Pagefault, false)
                .unwrap();
        }
    }
}

#[cfg_attr(test, automock)]
pub trait KVMIntrospectable {
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
    fn pause(&self) -> Result<(), std::io::Error>;
    fn get_vcpu_count(&self) -> Result<u32, std::io::Error>;
    fn get_page_access(&self, gpa: u64) -> Result<u8, std::io::Error>;
    fn set_page_access(&self, _gpa: u64, _access: u8) -> Result<(), std::io::Error> {
        unimplemented!()
    }
    fn get_registers(&self, vcpu: u16) -> Result<(kvm_regs, kvm_sregs, KvmMsr), std::io::Error>;
    fn set_registers(&self, vcpu: u16, regs: &KVMiRegs) -> Result<(), std::io::Error>;
    fn wait_and_pop_event(&self, ms: i32) -> Result<Option<KVMiEvent>, std::io::Error>;
    fn reply(&self, event: &KVMiEvent, reply_type: KVMiEventReply) -> Result<(), std::io::Error>;
    fn get_maximum_gfn(&self) -> Result<u64, std::io::Error>;
}

pub struct KVMiWrapper {
    kvmi: KVMi,
}

impl KVMiWrapper {
    pub fn new(kvmi: KVMi) -> KVMiWrapper {
        KVMiWrapper { kvmi }
    }
}

impl KVMIntrospectable for KVMiWrapper {
    fn init(&mut self, socket_path: &str) -> Result<(), std::io::Error> {
        self.kvmi.init(socket_path)
    }

    fn control_events(
        &self,
        vcpu: u16,
        intercept_type: KVMiInterceptType,
        enabled: bool,
    ) -> Result<(), std::io::Error> {
        self.kvmi.control_events(vcpu, intercept_type, enabled)
    }

    fn control_cr(&self, vcpu: u16, reg: KVMiCr, enabled: bool) -> Result<(), std::io::Error> {
        self.kvmi.control_cr(vcpu, reg, enabled)
    }

    fn control_msr(&self, vcpu: u16, reg: u32, enabled: bool) -> Result<(), std::io::Error> {
        self.kvmi.control_msr(vcpu, reg, enabled)
    }

    fn read_physical(&self, gpa: u64, buffer: &mut [u8]) -> Result<(), std::io::Error> {
        self.kvmi.read_physical(gpa, buffer)
    }

    fn write_physical(&self, gpa: u64, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.kvmi.write_physical(gpa, buffer)
    }

    fn get_page_access(&self, gpa: u64) -> Result<u8, std::io::Error> {
        self.kvmi.get_page_access(gpa)
    }

    fn set_page_access(&self, gpa: u64, access: u8) -> Result<(), std::io::Error> {
        self.kvmi.set_page_access(gpa, access)
    }

    fn pause(&self) -> Result<(), std::io::Error> {
        self.kvmi.pause()
    }

    fn get_vcpu_count(&self) -> Result<u32, std::io::Error> {
        self.kvmi.get_vcpu_count()
    }

    fn get_registers(&self, vcpu: u16) -> Result<(kvm_regs, kvm_sregs, KvmMsr), std::io::Error> {
        self.kvmi.get_registers(vcpu)
    }

    fn set_registers(&self, vcpu: u16, regs: &KVMiRegs) -> Result<(), std::io::Error> {
        self.kvmi.set_registers(vcpu, regs)
    }

    fn wait_and_pop_event(&self, ms: i32) -> Result<Option<KVMiEvent>, std::io::Error> {
        self.kvmi.wait_and_pop_event(ms)
    }

    fn reply(&self, event: &KVMiEvent, reply_type: KVMiEventReply) -> Result<(), std::io::Error> {
        self.kvmi.reply(event, reply_type)
    }

    fn get_maximum_gfn(&self) -> Result<u64, std::io::Error> {
        self.kvmi.get_maximum_gfn()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::{eq, function};
    use test_case::test_case;

    #[test]
    fn test_fail_to_create_kvm_driver_if_kvmi_init_returns_error() {
        let mut kvmi_mock = MockKVMIntrospectable::default();
        kvmi_mock.expect_init().returning(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });

        let result = Kvm::new("some_vm", Box::new(kvmi_mock));

        assert!(result.is_err(), "Expected error, got ok instead!");
    }

    /* #[test_case(1; "single vcpu")]
    fn test_pause_vcpu(vcpu_count: u32) {
        let mut kvmi_mock = MockKVMIntrospectable::default();
        kvmi_mock.expect_init().returning(|_| Ok(()));
        kvmi_mock.expect_pause().returning(|| Ok(()));
        kvmi_mock.expect_get_vcpu_count().returning(move || Ok(vcpu_count));
        let mut kvm = Kvm::new("some_vm", Box::new(kvmi_mock)).expect("failed to create driver");
        let result = Kvm::pause(&mut kvm);
        assert!(result.is_ok());
    }*/

    #[test_case(1; "single vcpu")]
    fn test_create_kvm_driver_if_guest_domain_is_valid(vcpu_count: u32) {
        let mut kvmi_mock = MockKVMIntrospectable::default();
        kvmi_mock.expect_init().returning(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "something went wrong",
            ))
        });
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
                .returning(|_, _, _| Ok(()));
            kvmi_mock
                .expect_control_events()
                .with(
                    eq(vcpu as u16),
                    function(|x| matches!(x, KVMiInterceptType::Cr)),
                    eq(false),
                )
                .returning(|_, _, _| Ok(()));
        }

        let result = Kvm::new("some_vm", Box::new(kvmi_mock));

        //assert!(result.is_ok(), "Expected ok, got error instead!");
    }
}
