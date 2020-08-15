use std::convert::TryInto;
use std::error::Error;
use std::ffi::{CStr, IntoStringError};
use std::os::raw::c_char;

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

/// Supports passing initialization parameters to the driver
///
/// Some drivers can support optional extra initialization parameters.
///
/// This is required to initialize the KVM driver, which needs a `domain_name` and
/// a `kvm_socket` parameters.
///
/// This is equivalent to LibVMI's `vmi_init_data_type_t`
#[repr(C)]
#[derive(Debug)]
pub enum DriverInitParam {
    KVMiSocket(String),
}

impl TryInto<DriverInitParam> for DriverInitParamFFI {
    type Error = IntoStringError;

    fn try_into(self) -> Result<DriverInitParam, Self::Error> {
        Ok(match self {
            DriverInitParamFFI::KVMiSocket(cstr_socket) => DriverInitParam::KVMiSocket(
                unsafe { CStr::from_ptr(cstr_socket) }
                    .to_owned()
                    .into_string()?,
            ),
        })
    }
}

/// Support passing initialization options
/// similar to DriverInitParam, however this enum offers C API compatibility
#[repr(C)]
#[derive(Debug)]
pub enum DriverInitParamFFI {
    KVMiSocket(*const c_char),
}

#[repr(C)]
#[derive(Debug)]
pub struct SegmentReg {
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
    pub cs: SegmentReg,
    pub ds: SegmentReg,
    pub es: SegmentReg,
    pub fs: SegmentReg,
    pub gs: SegmentReg,
    pub ss: SegmentReg,
    pub tr: SegmentReg,
    pub ldt: SegmentReg,
}

#[repr(C)]
#[derive(Debug)]
pub enum Registers {
    X86(X86Registers),
}

pub const PAGE_SHIFT: u32 = 12;
pub const PAGE_SIZE: u32 = 4096;

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

    fn write_registers(&self, _vcpu: u16, _reg: Registers) -> Result<(), Box<dyn Error>> {
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
}

// Event handling
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum InterceptType {
    Cr(CrType),
    Msr(u32),
    Breakpoint,
}

#[repr(C)]
#[derive(Debug)]
pub enum EventType {
    Cr { cr_type: CrType, new: u64, old: u64 },
    Msr { msr_type: u32, new: u64, old: u64 },
    Breakpoint { gpa: u64, insn_len: u8 },
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum CrType {
    Cr0,
    Cr3,
    Cr4,
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
