use std::convert::TryInto;
use std::error::Error;
use std::ffi::{CStr, IntoStringError};

use enum_iterator::IntoEnumIterator;

use registers::Registers;

use crate::capi::DriverInitParamFFI;

pub mod params;
pub mod registers;

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
#[derive(Debug, Copy, Clone, PartialEq, IntoEnumIterator)]
pub enum DriverType {
    KVM,
    VirtualBox,
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
#[derive(Debug, Clone)]
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
    /// * 'bytes_read' - the number of bytes read
    ///
    fn read_physical(
        &self,
        _paddr: u64,
        _buf: &mut [u8],
        _bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Modify contents of physical memory, starting at paddr, from buf
    ///
    /// # Arguments
    ///
    /// * 'paddr' - the physical address to write into
    /// * 'buf' - the data to be written into memory
    ///
    fn write_physical(&self, _paddr: u64, _buf: &[u8]) -> Result<(), Box<dyn Error>> {
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

    /// Return the concrete DriverType
    fn get_driver_type(&self) -> DriverType;
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
#[derive(Debug, Copy, Clone, PartialEq)]
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
