use std::convert::TryInto;
use std::error::Error;
use std::mem;
use std::vec::Vec;

use kvmi::{KVMi, KVMiCr, KVMiEvent, KVMiEventReply, KVMiEventType, KVMiInterceptType, KVMiMsr};

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

        // enable CR event intercept by default
        // (interception will take place when CR register will be specified)
        for vcpu in 0..vcpu_count {
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Cr, true)
                .unwrap();
            kvm.kvmi
                .control_events(vcpu, KVMiInterceptType::Msr, true)
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

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        let max_gfn = self.kvmi.get_maximum_gfn()?;
        Ok(max_gfn << PAGE_SHIFT)
    }

    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let (regs, _sregs, _msrs) = self.kvmi.get_registers(vcpu)?;
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
            cr0: _sregs.cr0,
            cr2: _sregs.cr2,
            cr3: _sregs.cr3,
            cr4: _sregs.cr4,
            sysenter_cs: _msrs.entries[0].data,
            sysenter_esp: _msrs.entries[1].data,
            sysenter_eip: _msrs.entries[2].data,
            msr_efer: _msrs.entries[3].data,
            msr_star: _msrs.entries[4].data,
            msr_lstar: _msrs.entries[5].data,
            efer: _sregs.efer,
            apic_base: _sregs.apic_base,
            cs: segment_reg {
                base: _sregs.cs.base,
                limit: _sregs.cs.limit,
                selector: _sregs.cs.selector,
            },
            ds: segment_reg {
                base: _sregs.ds.base,
                limit: _sregs.ds.limit,
                selector: _sregs.ds.selector,
            },
            es: segment_reg {
                base: _sregs.es.base,
                limit: _sregs.es.limit,
                selector: _sregs.es.selector,
            },
            fs: segment_reg {
                base: _sregs.fs.base,
                limit: _sregs.fs.limit,
                selector: _sregs.fs.selector,
            },
            gs: segment_reg {
                base: _sregs.gs.base,
                limit: _sregs.gs.limit,
                selector: _sregs.gs.selector,
            },
            ss: segment_reg {
                base: _sregs.ss.base,
                limit: _sregs.ss.limit,
                selector: _sregs.ss.selector,
            },
            tr: segment_reg {
                base: _sregs.tr.base,
                limit: _sregs.tr.limit,
                selector: _sregs.tr.selector,
            },
            ldt: segment_reg {
                base: _sregs.ldt.base,
                limit: _sregs.ldt.limit,
                selector: _sregs.ldt.selector,
            },
        }))
    }

    fn write_registers(&self, vcpu: u16, value: u64, reg: Register) -> Result<(), Box<dyn Error>> {
        let (mut regs, _sregs, _msrs) = self.kvmi.get_registers(vcpu)?;
        match reg {
            a if a == Register::RAX => regs.rax = value,
            a if a == Register::RBX => regs.rbx = value,
            a if a == Register::RCX => regs.rcx = value,
            a if a == Register::RDX => regs.rdx = value,
            a if a == Register::RSI => regs.rsi = value,
            a if a == Register::RDI => regs.rdi = value,
            a if a == Register::RSP => regs.rsp = value,
            a if a == Register::RBP => regs.rbp = value,
            a if a == Register::R8 => regs.r8 = value,
            a if a == Register::R9 => regs.r9 = value,
            a if a == Register::R10 => regs.r10 = value,
            a if a == Register::R11 => regs.r11 = value,
            a if a == Register::R12 => regs.r12 = value,
            a if a == Register::R13 => regs.r13 = value,
            a if a == Register::R14 => regs.r14 = value,
            a if a == Register::R15 => regs.r15 = value,
            a if a == Register::RIP => regs.rip = value,
            a if a == Register::RFLAGS => regs.rflags = value,
            _ => println!("wrong choice"),
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
                    KVMiEventType::PauseVCPU => panic!("Unexpected PauseVCPU event. It should have been popped by resume VM. (Did you forget to resume your VM ?)"),
                    _ => panic!("event not supported yet"),
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
        }
    }
}
