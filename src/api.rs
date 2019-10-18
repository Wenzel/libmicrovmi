use std::error::Error;

#[derive(Debug)]
pub enum DriverType {
    Dummy,
    #[cfg(feature = "xen")]
    Xen,
    #[cfg(feature = "kvm")]
    KVM,
    #[cfg(feature = "virtualbox")]
    VirtualBox,
	#[cfg(feature="hyper-v")]
	HyperV
}

#[derive(Debug)]
pub struct X86Registers {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

#[derive(Debug)]
pub enum Registers {
    X86(X86Registers),
}

pub trait Introspectable {
    // read physical memory
    fn read_physical(&self, _paddr: u64, _buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    // get max physical address
    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        unimplemented!();
    }

    fn read_registers(&self, _vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        unimplemented!();
    }

    // pause the VM
    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    // resume the VM
    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }
}
