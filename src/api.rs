use std::convert::TryInto;
use std::error::Error;
use std::ffi::{CStr, IntoStringError};

use crate::capi::DriverInitParamFFI;

bitflags! {
    pub struct Access: u32 {
        const R=0b00000001;
        const W=0b00000010;
        const X=0b00000100;
        const NIL=0b00000000;
        const RW=Self::R.bits | Self::W.bits;
        const WX=Self::W.bits | Self::X.bits;
        const RX=Self::R.bits | Self::X.bits;
        const RWX=Self::R.bits | Self::W.bits | Self::X.bits;
    }
}

///Represents the available hypervisor VMI drivers supported by libmicrovmi
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

///an x86 segment register
#[repr(C)]
#[derive(Debug, Default)]
pub struct SegmentReg {
    ///Stores the base address of a code segment
    pub base: u64,
    ///Used as a threshold for offset. If offset added to base is more than limit, GP fault is generated. Otherwise a linear physical address is created.
    pub limit: u32,
    ///Represents 16 bit segment selector consisting of a 2-bit Requested Privilege Level (RPL), a 1-bit Table Indicator (TI), and a 13-bit index.
    pub selector: u16,
}

/// x86 System Table Registers
/// (GDTR, IDTR)
#[repr(C)]
#[derive(Debug, Default)]
pub struct SystemTableReg {
    /// 32/64 bits linear base address
    pub base: u64,
    /// 16 bits table limit
    pub limit: u16,
}

///Represents all x86 registers on a specific VCPU
#[repr(C)]
#[derive(Debug, Default)]
pub struct X86Registers {
    /// 8 byte general purpose register.
    pub rax: u64,
    /// 8 byte general purpose register.
    pub rbx: u64,
    /// 8 byte general purpose register.
    pub rcx: u64,
    /// 8 byte general purpose register.
    pub rdx: u64,
    /// 8 byte general purpose register.
    pub rsi: u64,
    /// 8 byte general purpose register.
    pub rdi: u64,
    /// 8 byte general purpose register.
    pub rsp: u64,
    /// 8 byte general purpose register.
    pub rbp: u64,
    /// 8 byte general purpose register.
    pub r8: u64,
    /// 8 byte general purpose register.
    pub r9: u64,
    /// 8 byte general purpose register.
    pub r10: u64,
    /// 8 byte general purpose register.
    pub r11: u64,
    /// 8 byte general purpose register.
    pub r12: u64,
    /// 8 byte general purpose register.
    pub r13: u64,
    /// 8 byte general purpose register.
    pub r14: u64,
    /// 8 byte general purpose register.
    pub r15: u64,
    /// 8 byte general purpose register.
    pub rip: u64,
    /// 8 byte general purpose register.
    pub rflags: u64,
    ///Has various control flags that modify the basic operation of the processor.
    pub cr0: u64,
    ///Contains a value called Page Fault Linear Address (PFLA). When a page fault occurs, the address the program attempted to access is stored in the CR2 register. CR2 register cannot be intercepted by the guest operating system.
    pub cr2: u64,
    ///CR3 enables the processor to translate linear addresses into physical addresses by locating the page directory and page tables for the current task. Typically, the upper 20 bits of CR3 become the page directory base register (PDBR), which stores the physical address of the first page directory entry.
    pub cr3: u64,
    ///Used in protected mode to control operations such as virtual-8086 support, enabling I/O breakpoints, page size extension and machine-check exceptions.
    pub cr4: u64,
    ///Contains the 32-bit segment selector for the privilege level 0 code segment. Its index value is 0x174.
    pub sysenter_cs: u64,
    ///Contains the 32-bit offset into the privilege level 0 code segment to the first instruction of the selected operating procedure or routine. Its index value is 0x175.
    pub sysenter_esp: u64,
    ///Contains the 32-bit stack pointer for the privilege level 0 stack. Its index value is 0x176.
    pub sysenter_eip: u64,
    ///Extended Feature Enable Register (EFER) allows enabling the SYSCALL/SYSRET instruction, and later for entering and exiting long mode. Its index value is 0xc0000080.
    pub msr_efer: u64,
    ///Used to set the handler for SYSCALL and /or SYSRET instructions used for system calls. Its index value is 0xc0000081.
    pub msr_star: u64,
    ///Used to set the handler for SYSCALL and /or SYSRET instructions used for system calls. Its index value is 0xc0000082.
    pub msr_lstar: u64,
    ///Extened Feature Enable Register
    pub efer: u64,
    ///Advanced Programmable Interrupt Control Register
    pub apic_base: u64,
    ///Code segment register
    pub cs: SegmentReg,
    ///Data segment register
    pub ds: SegmentReg,
    ///Extra segment register, customizable by the programmer.
    pub es: SegmentReg,
    ///Points to TIB(Thread Information block) of a process.
    pub fs: SegmentReg,
    ///Points to TLS(Thread Local Storage) of a process.
    pub gs: SegmentReg,
    ///Stack Segment Register
    pub ss: SegmentReg,
    /// Task Register
    pub tr: SegmentReg,
    /// Local descriptor table register
    pub ldt: SegmentReg,
    pub idt: SystemTableReg,
    pub gdt: SystemTableReg,
}

#[repr(C)]
#[derive(Debug)]
pub enum Registers {
    X86(X86Registers),
}

pub const PAGE_SHIFT: u32 = 12;
pub const PAGE_SIZE: u32 = 4096;

pub trait Introspectable {
    /// Retrieve the number of VCPUs.
    ///
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        unimplemented!();
    }

    /// read the physical memory, starting from paddr, into buf
    ///
    /// # Arguments
    ///
    /// * 'paddr' - the physical address to read from
    /// * 'buf' - the data read from memory
    ///
    fn read_physical(&self, _paddr: u64, _buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Modify contents of physical memory, starting at paddr, from buf
    ///
    /// # Arguments
    ///
    /// * 'paddr' - the physical address to write into
    /// * 'buf' - the data to be written into memory
    ///
    fn write_physical(&self, _paddr: u64, _buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Get the maximum physical address
    ///
    /// Returns maximum physical address in 64 bit unsigned integer format.
    ///
    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        unimplemented!();
    }

    /// Read register values
    ///
    /// # Arguments
    /// * 'vcpu' - vcpu id for which the value of registers are to be dumped as the argument
    ///
    fn read_registers(&self, _vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        unimplemented!();
    }

    ///get page access
    ///
    /// # Arguments
    /// * 'paddr' - physical address of the page whose access we want to know.
    ///
    fn get_page_access(&self, _paddr: u64) -> Result<Access, Box<dyn Error>> {
        unimplemented!();
    }

    ///set page access
    ///
    /// # Arguments
    /// * 'paddr' - physical address of the page whose access we want to set
    /// * 'access' - access flags to be set on the given page
    ///
    fn set_page_access(&self, _paddr: u64, _access: Access) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Write register values
    ///
    /// # Arguments
    /// * 'vcpu' - vcpu id for which the value of registers are to be set
    /// * 'reg' - Registers enum having values to be set
    ///
    fn write_registers(&self, _vcpu: u16, _reg: Registers) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to pause the VM
    ///
    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to resume the VM
    ///
    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to enable/disable an event interception
    ///
    /// # Arguments
    /// * 'vcpu' - vcpu id for which we are to enable/disable intercept monitoring
    /// * 'intercept_type' - to specify event type for which to raise flag
    /// * 'enabled' - flag to specify whether to enable/disable event monitoring
    ///
    fn toggle_intercept(
        &mut self,
        _vcpu: u16,
        _intercept_type: InterceptType,
        _enabled: bool,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Listen and return the next event, or None
    ///
    /// # Arguments
    /// * 'timeout' - Time for which it will wait for a new event
    ///
    fn listen(&mut self, _timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        unimplemented!();
    }

    /// Send reply corresponding to the current event being popped
    ///
    /// # Arguments
    /// * 'event'
    /// * 'reply_type'
    ///
    fn reply_event(
        &mut self,
        _event: Event,
        _reply_type: EventReplyType,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }
}

/// Various types of intercepts handled by libmicrovmi
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum InterceptType {
    /// Intercept when value of cr register is changed by the guest
    Cr(CrType),
    /// Intercept when value of msr register is changed by the guest
    Msr(u32),
    /// Intercept when guest requests an access to a page for which the requested type of access is not granted. For example , guest tries to write on a read only page.
    Breakpoint,
    Pagefault,
}

/// Various types of events along with their relevant attributes being handled by this driver
#[repr(C)]
#[derive(Debug)]
pub enum EventType {
    ///Cr register interception
    Cr {
        ///Type of control register
        cr_type: CrType,
        /// new value after cr register has been intercepted by the guest.
        new: u64,
        /// old value before cr register has been intercepted by the guest.
        old: u64,
    },
    ///Msr register interception
    Msr {
        ///Type of model specific register
        msr_type: u32,
        /// new value after msr register has been intercepted by the guest.
        value: u64,
    },
    ///int3 interception
    Breakpoint {
        /// Physical memory address of the guest
        gpa: u64,
        /// instruction length. Generally it should be one. Anything other than one implies malicious guest.
        insn_len: u8,
    },
    Pagefault {
        /// Virtual memory address of the guest
        gva: u64,
        /// Physical memory address of the guest
        gpa: u64,
        /// Acsess responsible for thr pagefault
        access: Access,
    },
}

///Types of x86 control registers are listed here
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum CrType {
    ///Has various control flags that modify the basic operation of the processor.
    Cr0,
    ///Contains a value called Page Fault Linear Address (PFLA). When a page fault occurs, the address the program attempted to access is stored in the CR2 register. CR2 register cannot be intercepted by the guest operating system.
    Cr3,
    ///Used in protected mode to control operations such as virtual-8086 support, enabling I/O breakpoints, page size extension and machine-check exceptions.
    Cr4,
}

///This provides an abstraction of event which the hypervisor reports and using which we introspect the guest
#[repr(C)]
pub struct Event {
    ///vcpu on which the event is detected
    pub vcpu: u16,
    /// Type of event detected
    pub kind: EventType,
}

///Reply provided to the hypervisor after detecting an event
#[repr(C)]
#[derive(Debug)]
pub enum EventReplyType {
    Continue,
}
