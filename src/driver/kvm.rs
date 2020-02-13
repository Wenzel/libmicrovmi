use std::convert::TryInto;
use std::error::Error;

use kvmi::{KVMi, KVMiCr, KVMiEvent, KVMiEventReply, KVMiEventType, KVMiInterceptType};

use crate::api::{
    CrType, DriverType, Event, EventReplyType, EventType, InterceptType, Introspectable, Registers,
    X86Registers,
};

// unit struct
#[derive(Debug)]
pub struct Kvm {
    kvmi: KVMi,
    expect_pause_ev: u32,
}

impl InterceptType {
    fn to_kvmi(self) -> KVMiInterceptType {
        match self {
            InterceptType::Cr(_micro_cr_type) => KVMiInterceptType::Cr,
        }
    }
}

impl Kvm {
    pub fn new(domain_name: &str) -> Self {
        let socket_path = "/tmp/introspector";
        debug!("init on {} (socket: {})", domain_name, socket_path);
        let kvm = Kvm {
            kvmi: KVMi::new(socket_path),
            expect_pause_ev: 0,
        };
        // enable CR event intercept by default
        // (interception will take place when CR register will be specified)
        let inter_cr3 = InterceptType::Cr(CrType::Cr3);
        kvm.kvmi
            .control_events(0, inter_cr3.to_kvmi(), true)
            .unwrap();
        kvm
    }
}

impl Introspectable for Kvm {
    type DriverEvent = KVMiEvent;

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        Ok(self.kvmi.read_physical(paddr, buf)?)
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        // No API in KVMi at the moment
        // fake 512MB
        let max_addr = 1024 * 1024 * 512;
        Ok(max_addr)
    }

    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let (regs, sregs, _msrs) = self.kvmi.get_registers(vcpu)?;
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
            cr3: sregs.cr3,
            cr4: sregs.cr4,
        }))
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
            self.kvmi.wait_event(1000)?;
            // pop
            let kvmi_event = self.kvmi.pop_event()?;
            match kvmi_event.ev_type {
                KVMiEventType::PauseVCPU => {
                    debug!("Received Pause Event");
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
        }
    }

    fn listen(&mut self, timeout: u32) -> Result<Option<Event<Self::DriverEvent>>, Box<dyn Error>> {
        // wait for next event and pop it
        debug!("wait for next event");
        if self.kvmi.wait_event(timeout.try_into().unwrap())?.is_none() {
            // no events
            return Ok(None);
        }
        debug!("Pop next event");
        let kvmi_event = self.kvmi.pop_event()?;

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
            KVMiEventType::PauseVCPU => panic!("Unexpected PauseVCPU event. It should have been poped by resume VM. (Did you forgot to resume your VM ?)"),
        };

        let vcpu = kvmi_event.vcpu;

        Ok(Some(Event {
            vcpu,
            kind: microvmi_event_kind,
            inner: kvmi_event,
        }))
    }

    fn reply_event(
        &mut self,
        event: &Event<Self::DriverEvent>,
        reply_type: EventReplyType,
    ) -> Result<(), Box<dyn Error>> {
        let kvm_reply_type = match reply_type {
            EventReplyType::Continue => KVMiEventReply::Continue,
        };
        // get kvmi_event from microvmi Event struct
        let kvmi_event: &KVMiEvent = &event.inner;
        Ok(self.kvmi.reply(&kvmi_event, kvm_reply_type)?)
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::KVM
    }
}

impl Drop for Kvm {
    fn drop(&mut self) {
        debug!("KVM driver close");
        let inter_cr3 = InterceptType::Cr(CrType::Cr3);
        self.kvmi
            .control_events(0, inter_cr3.to_kvmi(), false)
            .unwrap();
    }
}
