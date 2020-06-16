use std::convert::TryInto;
use std::error::Error;
use std::mem;
use std::vec::Vec;

use kvmi::{
    KVMi, KVMiCr, KVMiEvent, KVMiEventReply, KVMiEventType, KVMiInterceptType, KVMiMsr,
    KVMiPageAccess,
};

//use kvmi_sys::*;

use crate::api::*;

// unit struct
#[derive(Debug)]
pub struct Kvm {
    kvmi: KVMi,
    expect_pause_ev: u32,
    // VCPU -> KVMiEvent
    vec_events: Vec<Option<KVMiEvent>>,
}

impl Kvm {
    pub fn new(domain_name: &str) -> Self {
        let socket_path = "/tmp/introspector";
        debug!("init on {} (socket: {})", domain_name, socket_path);
        let mut kvm = Kvm {
            kvmi: KVMi::new(socket_path),
            expect_pause_ev: 0,
            vec_events: Vec::new(),
        };

        // set vec_events size
        let vcpu_count = kvm.get_vcpu_count().unwrap();
        kvm.vec_events
            .resize_with(vcpu_count.try_into().unwrap(), || None);

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

        kvm
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

    fn write_registers(&self, vcpu: u16, value: u64, reg: Register) -> Result<(), Box<dyn Error>> {
        let (mut regs, _sregs, _msrs) = self.kvmi.get_registers(vcpu)?;
        match reg {
            Register::RAX => regs.rax = value,
            Register::RBX => regs.rbx = value,
            Register::RCX => regs.rcx = value,
            Register::RDX => regs.rdx = value,
            Register::RSI => regs.rsi = value,
            Register::RDI => regs.rdi = value,
            Register::RSP => regs.rsp = value,
            Register::RBP => regs.rbp = value,
            Register::R8 => regs.r8 = value,
            Register::R9 => regs.r9 = value,
            Register::R10 => regs.r10 = value,
            Register::R11 => regs.r11 = value,
            Register::R12 => regs.r12 = value,
            Register::R13 => regs.r13 = value,
            Register::R14 => regs.r14 = value,
            Register::R15 => regs.r15 = value,
            Register::RIP => regs.rip = value,
            Register::RFLAGS => regs.rflags = value,
        }
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
                    CrType::Cr2 => KVMiCr::Cr2,
                    CrType::Cr3 => KVMiCr::Cr3,
                    CrType::Cr4 => KVMiCr::Cr4,
                };
                Ok(self.kvmi.control_cr(vcpu, kvmi_cr, enabled)?)
            }
            InterceptType::Msr(micro_msr_type) => {
                let kvmi_msr = match micro_msr_type {
                    MsrType::SysenterCs => KVMiMsr::SysenterCs,
                    MsrType::SysenterEsp => KVMiMsr::SysenterEsp,
                    MsrType::SysenterEip => KVMiMsr::SysenterEip,
                    MsrType::MsrStar => KVMiMsr::MsrStar,
                    MsrType::MsrLstar => KVMiMsr::MsrLstar,
                    MsrType::MsrEfer => KVMiMsr::MsrEfer,
                };
                Ok(self.kvmi.control_msr(vcpu, kvmi_msr, enabled)?)
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
                            KVMiCr::Cr2 => CrType::Cr2,
                            KVMiCr::Cr3 => CrType::Cr3,
                            KVMiCr::Cr4 => CrType::Cr4,
                        },
                        new,
                        old,
                    },
                    KVMiEventType::Msr { msr_type, new, old } => EventType::Msr {
                        msr_type: match msr_type {
                            KVMiMsr::SysenterCs => MsrType::SysenterCs,
                            KVMiMsr::SysenterEsp => MsrType::SysenterEsp,
                            KVMiMsr::SysenterEip => MsrType::SysenterEip,
                            KVMiMsr::MsrStar => MsrType::MsrStar,
                            KVMiMsr::MsrLstar => MsrType::MsrLstar,
                            KVMiMsr::MsrEfer => MsrType::MsrEfer,
                        },
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
        // disable all control register interception
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

/*mock! {
    KVMi
    {
        fn get_registers(&self, vcpu: u16) -> Result<(kvm_regs, kvm_sregs, kvm_msrs), std::io::Error>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvmi_sys::{
        kvm_msrs, kvm_regs, kvm_sregs, kvmi_dom_event, kvmi_event_cr_reply, kvmi_event_reply,
        kvmi_introspector2qemu, kvmi_qemu2introspector, kvmi_vcpu_hdr, KVMI_EVENT_CR,
        KVMI_EVENT_PAUSE_VCPU,
    };
    #[test]
    fn test_read_register() {
        let domain = "dummy";
        let mut mock = MockKVMi::new();
        mock.expect_get_registers().withf(|x: &u16| *x == vcpu);
        let mut k = Kvm::new(domain);
        const vcpu: u16 = 0;
        k.read_registers(vcpu).expect("failed to call function");
    }
}
#[test]
    fn test_write_register()
    {
        let domain="dummy";
        let mut mock=MockKVMi::new(domain);
        pub const vcpu: u16=0;
        pub const value: u64=0;
        pub const reg: Register=Register::RAX;
        mock.expect_get_registers().withf(|x: &u16| x==vcpu);
        mock.expect_set_registers().withf(|x: u16, y: &mut kvm_regs| *x==vcpu);
        write_registers(&mock,vcpu,value,reg);
    }
    #[test]
    fn test_read_physical()
    {
        let domain="dummy";
        let mut mock=MockKVMi::new(domain);
        pub const page_size: usize=4096;
        let mut buffer: [u8; page_size] = [0; page_size];
        let paddr: u64 = 0;
        mock.expect_read_physical().withf(|x: &u64, y: &mut [u8]| *x==paddr);
        read_physical(&mock,paddr,&mut buffer);

    }
    #[test]
    fn test_write_physical()
    {
        let domain="dummy";
        let mut mock=MockKVMi::new(domain);
        pub const page_size: usize=4096;
        let mut buffer: [u8; page_size] = [0; page_size];
        let paddr: u64 = 0;
        mock.expect_write_physical().withf(|x: &u64, y: &mut [u8]| *x==paddr);
        write_physical(&mock,paddr,&mut buffer);

    }
    #[test]
    fn test_cr_intercept()
    {
        let domain="dummy";
        let mut mock=MockKVMi::new(domain);
        let vcpu: u16 = 0;
        let flag: bool = true;
        let intercept_type=InterceptType::Cr(CrType::Cr3);
        mock.expect_control_cr().withf(|x: &u16,y: KVMiCr,z: &bool| *x==vcpu && y==KVMiCr::Cr3 && *z==flag);
        toggle_intercept(&mock,vcpu,intercept_type,flag);

    }
    #[test]
    fn test_msr_intercept()
    {
        let domain="dummy";
        let mut mock=MockKVMi::new(domain);
        let vcpu: u16 = 0;
        let flag: bool = true;
        let intercept_type=InterceptType::Msr(MsrType::SysenterCs);
        mock.expect_control_msr().withf(|x: &u16,y: KVMiMsr,z: &bool| *x==vcpu && y==KVMiMsr::SysenterCs && *z==flag);
        toggle_intercept(&mock,vcpu,intercept_type,flag);
    }
}*/
