use std::error::Error;


pub const RAX: u64 = 0;
pub const RBX: u64 = 1;
pub const RCX: u64 = 2;
pub const RDX: u64 = 3;
pub const RSI: u64 = 4;
pub const RDI: u64 = 5;
pub const RSP: u64 = 6;
pub const RBP: u64 = 7;
pub const R8: u64 = 8;
pub const R9: u64 = 9;
pub const R10: u64 = 10;
pub const R11: u64 = 11;
pub const R12: u64 = 12;
pub const R13: u64 = 13;
pub const R14: u64 = 14;
pub const R15: u64 = 15;
pub const RIP: u64 = 16;
pub const RFLAGS: u64 = 17;




#[repr(C)]
#[derive(Debug)]
pub enum DriverType {
    Dummy,
    #[cfg(feature = "hyper-v")]
    HyperV,
    #[cfg(feature = "kvm")]
    KVM,
    #[cfg(feature = "virtualbox")]
    VirtualBox,
    #[cfg(feature = "xen")]
    Xen,
}


#[repr(C)]
#[derive(Debug)]
pub struct segment_reg {
    pub base: u64,
    pub limit: u32,
    pub selector: u16,
}

#[repr(C)]
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
    pub cr0: u64,
    pub cr2: u64,
    pub cr3: u64,
    pub cr4: u64,
    pub sysenter_cs: u64,
    pub sysenter_esp: u64,
    pub sysenter_eip: u64,
    pub msr_efer: u64,
    pub msr_star: u64,
    pub msr_lstar: u64,
    pub efer: u64,
    pub apic_base: u64,
    pub cs: segment_reg,
    pub ds: segment_reg,
    pub es: segment_reg,
    pub fs: segment_reg,
    pub gs: segment_reg,
    pub ss: segment_reg,
    pub tr: segment_reg,
    pub ldt: segment_reg,

   
}

#[repr(C)]
#[derive(Debug)]
pub enum Registers {
    X86(X86Registers),
}



pub trait Introspectable {
    // get VCPU count
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        unimplemented!();
    }

    // read physical memory
    fn read_physical(&self, _paddr: u64, _buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    // write physical memory
    fn write_physical(&self, _paddr: u64, _buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }
    // get max physical address
    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        unimplemented!();
    }

    fn read_registers(&self, _vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        unimplemented!();
    }

    fn write_registers(&self, _vcpu: u16, value: u64, reg: u64) -> Result<(), Box<dyn Error>> {
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

    // toggle an event interception
    fn toggle_intercept(
        &mut self,
        _vcpu: u16,
        _intercept_type: InterceptType,
        _enabled: bool,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    // listen and return the next event, or return None
    fn listen(&mut self, _timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        unimplemented!();
    }

    fn reply_event(
        &mut self,
        _event: Event,
        _reply_type: EventReplyType,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    // Introduced for the sole purpose of C interoperability.
    // Should be deprecated as soon as more suitable solutions become available.
    fn get_driver_type(&self) -> DriverType;
}

// Event handling
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum InterceptType {
    Cr(CrType),
    Msr(MsrType),
}

#[repr(C)]
#[derive(Debug)]
pub enum EventType {
    Cr { cr_type: CrType, new: u64, old: u64 },
    Msr {msr_type: MsrType, new: u64, old: u64 },
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum CrType {
    Cr0,
    Cr2,
    Cr3,
    Cr4,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum MsrType 
{
    Sysenter_cs,
    Sysenter_esp,
    Sysenter_eip,
    Msr_star,
    Msr_lstar,
    Msr_efer,
}

#[repr(C)]
pub struct Event {
    pub vcpu: u16,
    pub kind: EventType,
}

#[repr(C)]
#[derive(Debug)]
pub enum EventReplyType {
    Continue,
}
