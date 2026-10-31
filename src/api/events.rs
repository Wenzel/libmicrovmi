use crate::api::Access;

/// Various types of intercepts handled by libmicrovmi
///
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum InterceptType {
    /// Intercept when value of cr register is changed by the guest
    Cr(CrType),
    /// Intercept when value of msr register is changed by the guest
    Msr(u32),
    /// Intercept int3 breakpoints executed by the guest
    Breakpoint,
    /// Intercept when guest requests an access to a page for which the requested type of access is not granted. For example , guest tries to write on a read only page.
    Pagefault,
    /// Intercept every instruction the vCPU executes: one event at each instruction boundary
    Singlestep,
    /// Intercept CPUID instructions before the vCPU resumes.
    Cpuid,
    /// Intercept debug exceptions (#DB) before reinjection
    Debug,
}

/// Various types of events along with their relevant attributes being handled by this driver
///
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Debug, Clone)]
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
        /// value of the msr register before the write. Xen does not report it: always 0 there.
        old: u64,
    },
    ///int3 interception
    Breakpoint {
        /// Physical memory address of the guest
        gpa: u64,
        /// instruction length. Generally it should be one. Anything other than one implies malicious guest.
        insn_len: u8,
    },
    /// Page access: the vCPU attempted an access its page's access restriction denies, reported
    /// before the access, RIP at the faulting instruction
    Pagefault {
        /// Virtual memory address of the guest, 0 if the hypervisor did not report one
        gva: u64,
        /// Physical memory address of the guest
        gpa: u64,
        /// Access attempted (VirtualBox: exactly one of R, W, X)
        access: Access,
    },
    /// Single step: the vCPU reached the next instruction boundary
    Singlestep {
        /// Guest physical address of the next instruction (at RIP), `u64::MAX` if RIP is not
        /// mapped (VirtualBox: a step onto a page the guest has not faulted in yet)
        gpa: u64,
    },
    /// CPUID: original EAX/ECX inputs, before execution; registers at the event contain
    /// the computed EAX/EBX/ECX/EDX outputs, and RIP still points to CPUID.
    Cpuid {
        leaf: u32,
        subleaf: u32,
        /// Actual VM-exit instruction length (2..=15 bytes, including any prefixes).
        insn_len: u8,
    },
    /// Debug exception (#DB): DR6 value at the exception; reinjected by default
    Debug { dr6: u32 },
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
#[derive(Clone)]
pub struct Event {
    ///vcpu on which the event is detected
    pub vcpu: u16,
    /// The vCPU's Active view.
    pub view: u16,
    /// Type of event detected
    pub kind: EventType,
    /// Registers at event time, if available (VBoxVMI protocol v10+); null if not available or not supported.
    /// cbindgen:field-type=const X86Registers*
    pub regs: Option<Box<crate::api::registers::X86Registers>>,
}

///Reply provided to the hypervisor after detecting an event
///
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Debug)]
// inline buffers keep the C layout plain (ADR-0007)
#[allow(clippy::large_enum_variant)]
pub enum EventReplyType {
    /// Resume the vCPU.
    ///
    /// `reinject`: deliver an intercepted breakpoint to the guest, as if it had not been
    /// intercepted. Without reinjection, the vCPU executes again from RIP, which is the int3's
    /// address at a breakpoint event: restore the original byte first. Reinjection on any other
    /// event is an error (ADR-0005).
    Continue { reinject: bool, view: u16 },
    /// Execute exactly one instruction in `immediate_view`, then switch to `next_view`
    /// without delivering a step event. Independent of the Singlestep intercept (VBoxVMI only).
    FastSingleStep { immediate_view: u16, next_view: u16 },
    /// Emulate the faulting instruction once, ignoring every page access restriction (page-access
    /// event only, ADR-0007); a REP string instruction runs all its iterations.
    Emulate { view: u16 },
    /// Like `Emulate`, but memory writes are discarded (page-access event only).
    EmulateNoWrite { view: u16 },
    /// Like `Emulate`, the instruction's memory reads returning `data[..size]` (page-access event
    /// only; `size` 1..=256).
    EmulateReadData {
        size: u32,
        data: [u8; 256],
        view: u16,
    },
    /// Execute `data` as the instruction at RIP instead of guest memory (breakpoint event only).
    EmulateInsn { data: [u8; 16], view: u16 },
    /// Execute the faulting instruction natively once, this vCPU's page access restrictions
    /// lifted for it, then restored without a step event (page-access event only, VBoxVMI only).
    /// The fallback for an emulation reply the hypervisor refuses; writes and reads are real.
    UnrestrictedStep { view: u16 },
}
