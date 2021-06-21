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
