use std::cmp::PartialEq;
use std::error::Error;

/// Integer values corresponding to various registers
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Register {
    /// 8 byte general purpose register.
    RAX = 0,
    /// 8 byte general purpose register.
    RBX = 1,
    /// 8 byte general purpose register.
    RCX = 2,
    /// 8 byte general purpose register.
    RDX = 3,
    /// 8 byte general purpose register.
    RSI = 4,
    /// 8 byte general purpose register.
    RDI = 5,
    /// 8 byte general purpose register.
    RSP = 6,
    /// 8 byte general purpose register.
    RBP = 7,
    /// 8 byte general purpose register.
    R8 = 8,
    /// 8 byte general purpose register.
    R9 = 9,
    /// 8 byte general purpose register.
    R10 = 10,
    /// 8 byte general purpose register.
    R11 = 11,
    /// 8 byte general purpose register.
    R12 = 12,
    /// 8 byte general purpose register.
    R13 = 13,
    /// 8 byte general purpose register.
    R14 = 14,
    /// 8 byte general purpose register.
    R15 = 15,
    /// 8 byte general purpose register.
    RIP = 16,
    /// 8 byte general purpose register.
    RFLAGS = 17,
}

pub const PAGE_SHIFT: u32 = 12;
pub const PAGE_SIZE: u32 = 4096;

///Bits set in the index correspond to access permission granted corresponding to that bit. For example, for index 3 as first and second bit are set, read and write permission are granted
pub const ACCESS_STR: [&str; 8] = ["---", "r--", "-w-", "rw-", "--x", "r-x", "-wx", "rwx"];

///Represents type of hypervisor on which the guest runs
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

///Abstraction to represent a segment register and its various attributes
#[repr(C)]
#[derive(Debug)]
pub struct segment_reg {
    ///Stores the base address of a code segment
    pub base: u64,
    ///Used as a threshold for offset. If offset added to base is more than limit, GP fault is generated. Otherwise a linear physical address is created.
    pub limit: u32,
    ///Represents 16 bit segment selector consisting of a 2-bit Requested Privilege Level (RPL), a 1-bit Table Indicator (TI), and a 13-bit index.
    pub selector: u16,
}

///Represent all the registers in x86 system
#[repr(C)]
#[derive(Debug)]
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
    ///-Contains the 32-bit offset into the privilege level 0 code segment to the first instruction of the selected operating procedure or routine. Its index value is 0x175.
    pub sysenter_esp: u64,
    ///-Contains the 32-bit stack pointer for the privilege level 0 stack. Its index value is 0x176.
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
    pub cs: segment_reg,
    ///Data segment register
    pub ds: segment_reg,
    ///Extra segment register, customizable by the programmer.
    pub es: segment_reg,
    ///Points to TIB(Thread Information block) of a process.
    pub fs: segment_reg,
    ///Points to TLS(Thread Local Storage) of a process.
    pub gs: segment_reg,
    ///Stack Segment Register
    pub ss: segment_reg,
    /// Task Register
    pub tr: segment_reg,
    /// Local descriptor table register
    pub ldt: segment_reg,
}

#[repr(C)]
#[derive(Debug)]
pub enum Registers {
    X86(X86Registers),
}

pub trait Introspectable {
    /// Get number of vcpus
    ///
    /// It does not take any parameter and returns the number of vcpus in 16 bit unsigned integer format.
    ///
    /// # Examples
    ///
    /// ```
    /// let domain_name="guest";
    /// let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);
    /// let total_vcpu_count: u16 = drv.get_vcpu_count().expect("Failed to get vcpu count");
    /// ```
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to read the contents of physical memory.
    ///
    /// It takes the physical address of memory from where we are to read and pointer to an array of suitable size where the contents thus read are to be dumped.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut buffer: [u8; PAGE_SIZE] = [0; PAGE_SIZE];
    /// let curr_addr=0; //some address
    /// drv.read_physical(cur_addr, &mut buffer);    
    /// ```
    fn read_physical(&self, _paddr: u64, _buf: &mut [u8]) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to modify contents of physical memory.
    ///
    /// It takes the physical address of memory from where we are to start writing and pointer to an array of suitable size whose contents we are to place in that particular address of physical memory.
    ///
    /// # Examples
    ///
    /// ```
    /// let n=100; //some size
    /// let mut buffer: [u8; n] = [5; n];// array to be dumped into the physical memory
    /// let curr_addr=0; //some address
    /// drv.write_physical(cur_addr, &mut buffer);    
    /// ```
    fn write_physical(&self, _paddr: u64, _buf: &[u8]) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    ///Used to get the current permission level of a page
    ///
    /// It takes the physical address of a page as the argument and returns the access of the page as a 3 bit number: first bit corresponds to read permission, second bit corresponds to write permission and third bit corresponds to execute permission (0 for granted and 1 for denied).
    /// # Examples
    ///
    /// ```
    /// let mut access: u8 = drv.get_page_access(cur_addr).unwrap();  
    /// ```
    fn get_page_access(&self, _paddr: u64) -> Result<u8, Box<dyn Error>> {
        unimplemented!();
    }

    ///Used to set the current permission level of a page
    ///
    /// It takes as argument the physical address of a page and access of the page as a 3 bit number: first bit corresponds to read permission, second bit corresponds to write permission and third bit corresponds to execute permission (0 for granted and 1 for denied).
    /// # Examples
    ///
    /// ```
    /// let access = 5; //read and execute permission granted.
    /// drv.set_page_access(cur_addr, access).expect("failed to set page access");
    /// ```
    fn set_page_access(&self, _paddr: u64, _access: u8) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to get max physical address
    ///
    /// It does not take any argument and returns maximum physical address in 64 bit unsigned integer format.
    /// # Examples
    ///
    /// ```
    /// let max_addr = drv.get_max_physical_addr().unwrap();
    /// ```
    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to handle page fault event  
    ///
    /// It resolves page fault setting the access bit of the page, the absence of which caused the page fault. Like if page fault occurred with execute access, it sets the third bit of the page in question to 1.
    /// # Examples
    ///
    /// ```
    /// let gpa = 0x71a7d89; // some address
    /// let access = 7; //rwx access
    /// drv.handle_pf_event(gpa, access).expect("failed to resolve pagefault");
    /// ```
    fn handle_pf_event(&self, _paddr: u64, _pf_access: u8) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to read register values  
    ///
    /// Takes vcpu number for which the value of registers are to be dumped as the argument, and returns Registers enum containing the read values.
    ///  # Examples
    /// ```
    /// let vcpu = 0; // some vcpu number
    /// let regs = drv.read_registers(vcpu).expect("Failed to read registers");
    /// ```
    fn read_registers(&self, _vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to write register values  
    ///
    /// Takes vcpu number, value to be set and the register whose value should be set as the argument.
    ///  # Examples
    /// ```
    /// let vcpu = 0; // some vcpu number
    /// drv.write_registers(vcpu, 0x5, Registers::X86(Input)).expect("Failed to write registers");
    /// ```  
    fn write_registers(
        &self,
        _vcpu: u16,
        _reg: Registers,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to pause the VM
    ///
    ///  # Examples
    /// ```
    /// drv.pause().expect("Failed to pause vm");
    /// ```
    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to resume the VM
    ///
    ///  # Examples
    /// ```
    /// drv.resume().expect("Failed to resume vm");
    /// ```
    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Used to enable/disable an event interception
    ///
    /// It takes vcpu, intercept_type(of type enum InterceptType) and a flag(to enable/disable an event interception) as argument.
    ///
    ///  # Examples
    /// ```
    /// drv.toggle_intercept(vcpu, intercept, enabled).expect(&format!("Failed to enable page faults"));
    /// ```
    fn toggle_intercept(
        &mut self,
        _vcpu: u16,
        _intercept_type: InterceptType,
        _enabled: bool,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Listen and return the next event, or return None
    ///
    /// It takes timeout as its only argument. (Time for which it will wait for a new event)
    /// # Examples
    /// ```
    /// let timeout = 1000;
    /// let event = drv.listen(timeout).expect("Failed to listen for events");
    /// ```
    fn listen(&mut self, _timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        unimplemented!();
    }

    /// Send reply corresponding to the current event being popped
    ///
    /// It takes event struct and event reply type as its argument.
    /// # Examples
    /// ```
    /// let ev = Event {vcpu: 0, kind =Cr {old: 0x0, new: 0x5 }};
    /// drv.reply_event(ev, EventReplyType::Continue).expect("Failed to send event reply");
    /// ```
    fn reply_event(
        &mut self,
        _event: Event,
        _reply_type: EventReplyType,
    ) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Introduced for the sole purpose of C interoperability.
    /// Should be deprecated as soon as more suitable solutions become available.
    fn get_driver_type(&self) -> DriverType;
}

/// Various types of intercepts being handled by this driver
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum InterceptType {
    /// Intercept when value of cr register is changed by the guest
    Cr(CrType),
    /// Intercept when value of msr register is changed by the guest
    Msr(u32),
    /// Intercept when a int3 instruction is encountered
    Breakpoint,
    /// Intercept when guest requests an access to a page for which the requested type of access is not granted. For example , guest tries to write on a read only page.
    Pagefault,
}

/// Various types of events along with their relevant attributes being handled by this driver
#[repr(C)]
#[derive(Debug)]
pub enum EventType {
    Cr {
        ///Type of control register
        cr_type: CrType,
        /// new value after cr register has been intercepted by the guest.
        new: u64,
        /// old value before cr register has been intercepted by the guest.
        old: u64,
    },

    Msr {
        ///Type of model specific register
        msr_type: u32,
        /// new value after cr register has been intercepted by the guest.
        new: u64,
        /// old value before cr register has been intercepted by the guest.
        old: u64,
    },
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
        /// access requested by the guest for which page fault occurs
        access: u8,
        ///view
        view: u16,
    },
}

///Types of x86 control registers are listed here
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum CrType {
    ///Has various control flags that modify the basic operation of the processor.
    Cr0,
    ///CR3 enables the processor to translate linear addresses into physical addresses by locating the page directory and page tables for the current task. Typically, the upper 20 bits of CR3 become the page directory base register (PDBR), which stores the physical address of the first page directory entry.
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
