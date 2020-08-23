use std::error::Error;

use fdp::{RegisterType, FDP};

use crate::api::{DriverInitParam, Introspectable, Registers, X86Registers};

// unit struct
#[derive(Debug)]
pub struct VBox {
    fdp: FDP,
}

impl VBox {
    pub fn new(domain_name: &str, _init_option: Option<DriverInitParam>) -> Self {
        // init FDP
        let fdp = FDP::new(domain_name);
        VBox { fdp }
    }
}

impl Introspectable for VBox {
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        // no API to fetch VCPU count, hardcode to 1 for now
        Ok(1)
    }

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        self.fdp.read_physical_memory(paddr, buf)
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        self.fdp.get_physical_memory_size()
    }

    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let fdp_vcpu = vcpu as u32;
        let regs = X86Registers {
            rax: self.fdp.read_register(fdp_vcpu, RegisterType::RAX)?,
            rbx: self.fdp.read_register(fdp_vcpu, RegisterType::RBX)?,
            rcx: self.fdp.read_register(fdp_vcpu, RegisterType::RCX)?,
            rdx: self.fdp.read_register(fdp_vcpu, RegisterType::RDX)?,
            rsi: self.fdp.read_register(fdp_vcpu, RegisterType::RSI)?,
            rdi: self.fdp.read_register(fdp_vcpu, RegisterType::RDI)?,
            rbp: self.fdp.read_register(fdp_vcpu, RegisterType::RBP)?,
            rsp: self.fdp.read_register(fdp_vcpu, RegisterType::RSP)?,
            r8: self.fdp.read_register(fdp_vcpu, RegisterType::R8)?,
            r9: self.fdp.read_register(fdp_vcpu, RegisterType::R9)?,
            r10: self.fdp.read_register(fdp_vcpu, RegisterType::R10)?,
            r11: self.fdp.read_register(fdp_vcpu, RegisterType::R11)?,
            r12: self.fdp.read_register(fdp_vcpu, RegisterType::R12)?,
            r13: self.fdp.read_register(fdp_vcpu, RegisterType::R13)?,
            r14: self.fdp.read_register(fdp_vcpu, RegisterType::R14)?,
            r15: self.fdp.read_register(fdp_vcpu, RegisterType::R15)?,
            rip: self.fdp.read_register(fdp_vcpu, RegisterType::RIP)?,
            ..Default::default()
        };
        Ok(Registers::X86(regs))
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        self.fdp.pause()
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        self.fdp.resume()
    }
}
