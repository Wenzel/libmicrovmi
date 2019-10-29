use std::error::Error;
use crate::api::{Introspectable, Registers, X86Registers};
use kvmi::{KVMi, KVMiEventType};

// unit struct
#[derive(Debug)]
pub struct Kvm {
    kvmi: KVMi,
    expect_pause_ev: u32,
}

impl Kvm {

    pub fn new(domain_name: &str) -> Self {
        println!("KVM driver init on {}", domain_name);
        let socket_path = "/tmp/introspector";
        Kvm {
            kvmi: KVMi::new(socket_path),
            expect_pause_ev: 0,
        }
    }

    fn close(&mut self) {
        println!("KVM driver close");
    }
}

impl Introspectable for Kvm {

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),Box<dyn Error>> {
        Ok(self.kvmi.read_physical(paddr, buf)?)
    }

    fn get_max_physical_addr(&self) -> Result<u64,Box<dyn Error>> {
        // No API in KVMi at the moment
        // fake 512MB
        let max_addr = 1024 * 1024 * 512;
        Ok(max_addr)
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
            r8:  regs.r8,
            r9:  regs.r9,
            r10: regs.r10,
            r11: regs.r11,
            r12: regs.r12,
            r13: regs.r13,
            r14: regs.r14,
            r15: regs.r15,
            rip: regs.rip,
            rflags: regs.rflags,
        }))
    }

    fn pause(&mut self) -> Result<(),Box<dyn Error>> {
        println!("KVM driver pause");
        // already paused ?
        if self.expect_pause_ev > 0 {
            return Ok(());
        }

        self.kvmi.pause()?;
        self.expect_pause_ev = self.kvmi.get_vcpu_count()?;
        println!("expected pause events: {}", self.expect_pause_ev);
        Ok(())
    }

    fn resume(&mut self) -> Result<(),Box<dyn Error>> {
        println!("KVM driver resume");
        // already resumed ?
        if self.expect_pause_ev == 0 {
            return Ok(());
        }

        while self.expect_pause_ev > 0 {
            // wait
            self.kvmi.wait_event(1000)?;
            // pop
            let kvmi_event = self.kvmi.pop_event()?;
            match kvmi_event.kind {
                KVMiEventType::PauseVCPU => {
                    println!("Received Pause Event");
                    self.expect_pause_ev -= 1;
                    self.kvmi.reply_continue(&kvmi_event)?;
                }
                _ => panic!("Unexpected {:?} event type while resuming VM", kvmi_event.kind),
            }
        }
        Ok(())
    }

}

impl Drop for Kvm {
    fn drop(&mut self) {
        self.close();
    }
}

