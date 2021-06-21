use crate::api::Access;

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
