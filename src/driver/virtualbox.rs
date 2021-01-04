use std::error::Error;

use fdp::{RegisterType, FDP};

use crate::api::{
    DriverError, DriverInitParam, Introspectable, Registers, SegmentReg, SystemTableReg,
    X86Registers,
};

#[derive(thiserror::Error, Debug)]
pub enum VirtualBoxDriverError {
    #[error(transparent)]
    OtherError(#[from] Box<dyn Error>),
}

// unit struct
#[derive(Debug)]
pub struct VBox {
    fdp: FDP,
}

impl VBox {
    pub fn new(
        domain_name: &str,
        _init_option: Option<DriverInitParam>,
    ) -> Result<Self, DriverError> {
        // init FDP
        let fdp = FDP::new(domain_name);
        Ok(VBox { fdp })
    }
}

impl Introspectable for VBox {
    fn get_vcpu_count(&self) -> Result<u16, DriverError> {
        // no API to fetch VCPU count, hardcode to 1 for now
        Ok(1)
    }

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(), DriverError> {
        self.fdp
            .read_physical_memory(paddr, buf)
            .map_err(|err| VirtualBoxDriverError::OtherError(err).into())
    }

    fn get_max_physical_addr(&self) -> Result<u64, DriverError> {
        self.fdp
            .get_physical_memory_size()
            .map_err(|err| VirtualBoxDriverError::OtherError(err).into())
    }

    fn read_registers(&self, vcpu: u16) -> Result<Registers, DriverError> {
        let fdp_vcpu = vcpu as u32;
        let regs = X86Registers {
            rax: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RAX)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            rbx: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RBX)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            rcx: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RCX)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            rdx: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RDX)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            rsi: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RSI)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            rdi: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RDI)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            rbp: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RBP)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            rsp: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RSP)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            r8: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::R8)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            r9: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::R9)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            r10: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::R10)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            r11: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::R11)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            r12: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::R12)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            r13: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::R13)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            r14: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::R14)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            r15: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::R15)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            rip: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::RIP)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            cr0: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::CR0)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            cr2: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::CR2)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            cr3: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::CR3)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            cr4: self
                .fdp
                .read_register(fdp_vcpu, RegisterType::CR4)
                .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
            cs: SegmentReg {
                base: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::CS)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
                ..Default::default()
            },
            ds: SegmentReg {
                base: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::DS)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
                ..Default::default()
            },
            es: SegmentReg {
                base: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::ES)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
                ..Default::default()
            },
            fs: SegmentReg {
                base: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::FS)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
                ..Default::default()
            },
            gs: SegmentReg {
                base: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::GS)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
                ..Default::default()
            },
            ss: SegmentReg {
                base: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::SS)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
                ..Default::default()
            },
            gdt: SystemTableReg {
                base: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::GDTR_BASE)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
                limit: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::GDTR_LIMIT)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?
                    as u16,
            },
            idt: SystemTableReg {
                base: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::IDTR_BASE)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?,
                limit: self
                    .fdp
                    .read_register(fdp_vcpu, RegisterType::IDTR_LIMIT)
                    .map_err(|err| DriverError::from(VirtualBoxDriverError::OtherError(err)))?
                    as u16,
            },
            ..Default::default()
        };
        Ok(Registers::X86(regs))
    }

    fn pause(&mut self) -> Result<(), DriverError> {
        self.fdp
            .pause()
            .map_err(|err| VirtualBoxDriverError::OtherError(err).into())
    }

    fn resume(&mut self) -> Result<(), DriverError> {
        self.fdp
            .resume()
            .map_err(|err| VirtualBoxDriverError::OtherError(err).into())
    }
}
