//! VirtualBox driver, VBoxVMI access method: the VBoxVMI protocol of a patched VirtualBox.
use std::cell::RefCell;
use std::convert::TryFrom;
use std::error::Error;
use std::path::Path;
use std::time::Duration;

use vboxvmi::{Client, EventKind, InterceptKind, Reply, Segment, TableRegister};

use crate::api::events::{CrType, Event, EventReplyType, EventType, InterceptType};
use crate::api::registers::{Registers, SegmentReg, SystemTableReg, X86Registers};
use crate::api::{Access, DriverType, Introspectable};
use crate::driver::EferMismatch;

#[derive(thiserror::Error, Debug)]
pub enum VBoxVmiDriverError {
    #[error("VirtualBox driver, VBoxVMI method: guest physical page {addr:#x} is unreadable (VirtualBox status {rc})")]
    Unreadable { addr: u64, rc: i32 },
    #[error("VirtualBox driver, VBoxVMI method: guest physical page {addr:#x} is unwritable (VirtualBox status {rc})")]
    Unwritable { addr: u64, rc: i32 },
    #[error("VirtualBox driver, VBoxVMI method: CR write event for CR{0}, not CR0, CR3 or CR4")]
    UnexpectedCr(u32),
    #[error("VirtualBox driver, VBoxVMI method: unexpected page access {0:#x}")]
    UnexpectedAccess(u32),
    #[error("VirtualBox driver, VBoxVMI method: emulation read data size {0}, not 1 to 256")]
    ReadDataSize(u32),
}

fn segment_reg(s: &Segment) -> SegmentReg {
    SegmentReg {
        base: s.base,
        limit: s.limit,
        selector: s.selector,
    }
}

fn segment(s: &SegmentReg) -> Segment {
    Segment {
        base: s.base,
        limit: s.limit,
        selector: s.selector,
    }
}

fn system_table_reg(t: &TableRegister) -> SystemTableReg {
    SystemTableReg {
        base: t.base,
        limit: t.limit,
    }
}

fn table_register(t: &SystemTableReg) -> TableRegister {
    TableRegister {
        base: t.base,
        limit: t.limit,
    }
}

pub struct VBoxVmi {
    /// `RefCell`: requests need `&mut Client`, reads only `&self`.
    client: RefCell<Client>,
    /// End of the highest RAM range, fixed for the VM's lifetime.
    max_addr: u64,
    /// Fixed for the VM's lifetime.
    vcpu_count: u16,
}

impl VBoxVmi {
    /// Connects to the VM's VBoxVMI socket and performs the handshake.
    pub fn new(socket_path: &Path) -> Result<Self, Box<dyn Error>> {
        let mut client = Client::connect(socket_path)?;
        let max_addr = client
            .ram_ranges()?
            .last()
            .expect("vboxvmi::Client::ram_ranges is never empty")
            .end;
        let vcpu_count = u16::try_from(client.vcpu_count()?)?;
        Ok(VBoxVmi {
            client: RefCell::new(client),
            max_addr,
            vcpu_count,
        })
    }
}

/// Whether `e`, from [`VBoxVmi::new`], means the socket file does not exist: VBoxVMI is not
/// enabled on the VM (or it is not running).
#[cfg(feature = "virtualbox-api")]
pub fn is_socket_missing(e: &(dyn Error + 'static)) -> bool {
    matches!(e.downcast_ref(), Some(vboxvmi::Error::Connect { source, .. })
        if source.kind() == std::io::ErrorKind::NotFound)
}

fn access(bits: u32) -> Result<Access, VBoxVmiDriverError> {
    Access::from_bits(bits).ok_or(VBoxVmiDriverError::UnexpectedAccess(bits))
}

impl Introspectable for VBoxVmi {
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        Ok(self.vcpu_count)
    }

    /// On error, `bytes_read` is the length of the readable prefix, up to the first unreadable page.
    fn read_physical(
        &self,
        paddr: u64,
        buf: &mut [u8],
        bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        *bytes_read = 0;
        let mut status = Vec::new();
        let result = self
            .client
            .borrow_mut()
            .read_physical(&mut [(paddr, &mut *buf)], &mut status);

        let mut offset = 0usize;
        for rc in status {
            let addr = paddr + offset as u64;
            let page_len =
                (buf.len() - offset).min((vboxvmi::PAGE_SIZE - addr % vboxvmi::PAGE_SIZE) as usize);
            if rc < 0 {
                *bytes_read = offset as u64;
                return Err(VBoxVmiDriverError::Unreadable { addr, rc }.into());
            }
            offset += page_len;
        }
        *bytes_read = offset as u64;
        result?;
        debug_assert_eq!(offset, buf.len());
        Ok(())
    }

    /// A failed page leaves the successful prefix written; later pages may also have been written.
    fn write_physical(&self, paddr: u64, buf: &[u8]) -> Result<(), Box<dyn Error>> {
        let mut status = Vec::new();
        let result = self
            .client
            .borrow_mut()
            .write_physical(&[(paddr, buf)], &mut status);
        let mut offset = 0usize;
        for rc in status {
            let addr = paddr + offset as u64;
            if rc < 0 {
                return Err(VBoxVmiDriverError::Unwritable { addr, rc }.into());
            }
            offset +=
                (buf.len() - offset).min((vboxvmi::PAGE_SIZE - addr % vboxvmi::PAGE_SIZE) as usize);
        }
        result?;
        debug_assert_eq!(offset, buf.len());
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        Ok(self.max_addr)
    }

    /// One request, served between two instructions of the vCPU: a consistent snapshot of it,
    /// also while the VM runs. The MSRs `X86Registers` has (EFER, STAR, LSTAR, SYSENTER_*, APIC
    /// base) are all read, 0 if the guest CPU doesn't have one; `efer` and `msr_efer` are both
    /// EFER.
    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let r = self.client.borrow_mut().read_registers(vcpu.into())?;
        Ok(Registers::X86(X86Registers {
            rax: r.rax,
            rbx: r.rbx,
            rcx: r.rcx,
            rdx: r.rdx,
            rsi: r.rsi,
            rdi: r.rdi,
            rsp: r.rsp,
            rbp: r.rbp,
            r8: r.r8,
            r9: r.r9,
            r10: r.r10,
            r11: r.r11,
            r12: r.r12,
            r13: r.r13,
            r14: r.r14,
            r15: r.r15,
            rip: r.rip,
            rflags: r.rflags,
            cr0: r.cr0,
            cr2: r.cr2,
            cr3: r.cr3,
            cr4: r.cr4,
            sysenter_cs: r.sysenter_cs,
            sysenter_esp: r.sysenter_esp,
            sysenter_eip: r.sysenter_eip,
            msr_efer: r.efer,
            msr_star: r.star,
            msr_lstar: r.lstar,
            efer: r.efer,
            apic_base: r.apic_base,
            msr_cstar: r.cstar,
            msr_sfmask: r.sfmask,
            shadow_gs_base: r.kernel_gs_base,
            cs: segment_reg(&r.cs),
            ds: segment_reg(&r.ds),
            es: segment_reg(&r.es),
            fs: segment_reg(&r.fs),
            gs: segment_reg(&r.gs),
            ss: segment_reg(&r.ss),
            tr: segment_reg(&r.tr),
            ldt: segment_reg(&r.ldtr),
            idt: system_table_reg(&r.idtr),
            gdt: system_table_reg(&r.gdtr),
        }))
    }

    /// One request, applied between two instructions of the vCPU, all registers at once, also
    /// while the VM runs (pause to write several vCPUs at the same point). All or nothing: a value
    /// the CPU would not load (RFLAGS bit 1 clear or reserved bits set, reserved control register
    /// or EFER bits, an MSR value `WRMSR` refuses) fails the write and nothing is written.
    /// Registers keeping their value are not touched. Segment selectors, bases and limits are
    /// written as raw state (no descriptor table load, attributes kept). `efer` and `msr_efer`
    /// are both EFER: they must be equal.
    fn write_registers(&self, vcpu: u16, reg: Registers) -> Result<(), Box<dyn Error>> {
        let Registers::X86(r) = reg;
        EferMismatch::check(&r)?;
        let regs = vboxvmi::X86Registers {
            rax: r.rax,
            rcx: r.rcx,
            rdx: r.rdx,
            rbx: r.rbx,
            rsp: r.rsp,
            rbp: r.rbp,
            rsi: r.rsi,
            rdi: r.rdi,
            r8: r.r8,
            r9: r.r9,
            r10: r.r10,
            r11: r.r11,
            r12: r.r12,
            r13: r.r13,
            r14: r.r14,
            r15: r.r15,
            rip: r.rip,
            rflags: r.rflags,
            cr0: r.cr0,
            cr2: r.cr2,
            cr3: r.cr3,
            cr4: r.cr4,
            efer: r.efer,
            star: r.msr_star,
            lstar: r.msr_lstar,
            sysenter_cs: r.sysenter_cs,
            sysenter_esp: r.sysenter_esp,
            sysenter_eip: r.sysenter_eip,
            apic_base: r.apic_base,
            cstar: r.msr_cstar,
            sfmask: r.msr_sfmask,
            kernel_gs_base: r.shadow_gs_base,
            es: segment(&r.es),
            cs: segment(&r.cs),
            ss: segment(&r.ss),
            ds: segment(&r.ds),
            fs: segment(&r.fs),
            gs: segment(&r.gs),
            ldtr: segment(&r.ldt),
            tr: segment(&r.tr),
            gdtr: table_register(&r.gdt),
            idtr: table_register(&r.idt),
        };
        Ok(self
            .client
            .borrow_mut()
            .write_registers(vcpu.into(), &regs)?)
    }

    /// Synchronous and idempotent: one `resume` undoes any number of `pause`s.
    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(self.client.get_mut().pause()?)
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(self.client.get_mut().resume()?)
    }

    /// The allowed access of `paddr`'s 4K page in `view`, RWX if unrestricted.
    fn get_page_access(&self, view: u16, paddr: u64) -> Result<Access, Box<dyn Error>> {
        Ok(access(
            self.client.borrow_mut().get_page_access(view, paddr)?,
        )?)
    }

    /// Restricts `paddr`'s 4K RAM page in `view` to `access`; RWX lifts the restriction.
    fn set_page_access(
        &self,
        view: u16,
        paddr: u64,
        access: Access,
    ) -> Result<(), Box<dyn Error>> {
        Ok(self
            .client
            .borrow_mut()
            .set_page_access(view, paddr, access.bits())?)
    }

    fn enable_memory_views(&self, enabled: bool) -> Result<(), Box<dyn Error>> {
        Ok(self.client.borrow_mut().enable_memory_views(enabled)?)
    }

    fn create_memory_view(&self) -> Result<u16, Box<dyn Error>> {
        Ok(self.client.borrow_mut().create_memory_view()?)
    }

    fn destroy_memory_view(&self, view: u16) -> Result<(), Box<dyn Error>> {
        Ok(self.client.borrow_mut().destroy_memory_view(view)?)
    }

    fn switch_memory_view(&self, view: u16) -> Result<(), Box<dyn Error>> {
        Ok(self.client.borrow_mut().switch_memory_view(view)?)
    }

    fn remap_memory_page(
        &self,
        view: u16,
        page: u64,
        target: u64,
    ) -> Result<(), Box<dyn Error>> {
        Ok(self
            .client
            .borrow_mut()
            .remap_memory_page(view, page, target)?)
    }

    fn allocate_hidden_page(&self) -> Result<u64, Box<dyn Error>> {
        Ok(self.client.borrow_mut().allocate_hidden_page()?)
    }

    fn free_hidden_page(&self, page: u64) -> Result<(), Box<dyn Error>> {
        Ok(self.client.borrow_mut().free_hidden_page(page)?)
    }

    /// Breakpoint, single step, CPUID, CR0/CR3/CR4 write, MSR write (one MSR index per
    /// Intercept) and page access (`Pagefault`). Idempotent, per vCPU, also while paused or
    /// from within an event before its reply. Fails while a VirtualBox debugger is attached;
    /// CR, MSR, CPUID and page access Intercepts require VT-x (not AMD-V or NEM).
    fn toggle_intercept(
        &mut self,
        vcpu: u16,
        intercept_type: InterceptType,
        enabled: bool,
    ) -> Result<(), Box<dyn Error>> {
        let kind = match intercept_type {
            InterceptType::Breakpoint => InterceptKind::Breakpoint,
            InterceptType::Singlestep => InterceptKind::Singlestep,
            InterceptType::Cr(CrType::Cr0) => InterceptKind::CrWrite(0),
            InterceptType::Cr(CrType::Cr3) => InterceptKind::CrWrite(3),
            InterceptType::Cr(CrType::Cr4) => InterceptKind::CrWrite(4),
            InterceptType::Msr(index) => InterceptKind::MsrWrite(index),
            InterceptType::Pagefault => InterceptKind::PageAccess,
            InterceptType::Cpuid => InterceptKind::Cpuid,
            InterceptType::Debug => InterceptKind::Debug,
        };
        let client = self.client.get_mut();
        Ok(match enabled {
            true => client.set_intercept(vcpu.into(), kind),
            false => client.clear_intercept(vcpu.into(), kind),
        }?)
    }

    /// The oldest event of any vCPU, which stays parked until `reply_event`; the other vCPUs keep
    /// running. At a breakpoint event RIP is the int3's address. CR and MSR write events come
    /// before the write: the register still holds `old`, RIP is at the `MOV CRx`/`WRMSR`, which
    /// executes after the reply (CR3's `new` has bit 63, PCID no-flush, cleared). Page access
    /// events come before the access, RIP at the faulting instruction; `access` is the single
    /// access attempted, `gva` 0 when the CPU reported no linear address.
    /// CPUID events retain the original EAX/ECX inputs in `leaf`/`subleaf`; reading registers
    /// returns the computed CPUID outputs while RIP still points to the instruction. Continue
    /// advances RIP by the instruction length if unchanged and preserves any register overrides.
    fn listen(&mut self, timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        let event = match self
            .client
            .get_mut()
            .wait_event(Duration::from_millis(timeout.into()))?
        {
            Some(event) => event,
            None => return Ok(None),
        };
        let kind = match event.kind {
            EventKind::Breakpoint { gpa, insn_len } => EventType::Breakpoint {
                gpa,
                insn_len: u8::try_from(insn_len)?,
            },
            EventKind::Singlestep { gpa } => EventType::Singlestep { gpa },
            EventKind::CrWrite { cr, old, new } => EventType::Cr {
                cr_type: match cr {
                    0 => CrType::Cr0,
                    3 => CrType::Cr3,
                    4 => CrType::Cr4,
                    other => return Err(VBoxVmiDriverError::UnexpectedCr(other).into()),
                },
                new,
                old,
            },
            EventKind::MsrWrite { msr, old, new } => EventType::Msr {
                msr_type: msr,
                value: new,
                old,
            },
            EventKind::PageAccess {
                gpa,
                gva,
                access: bits,
            } => EventType::Pagefault {
                gva,
                gpa,
                access: access(bits)?,
            },
            EventKind::Cpuid {
                leaf,
                subleaf,
                insn_length,
            } => EventType::Cpuid {
                leaf,
                subleaf,
                insn_len: insn_length,
            },
            EventKind::Debug { dr6 } => EventType::Debug { dr6 },
        };
        Ok(Some(Event {
            vcpu: u16::try_from(event.vcpu)?,
            view: event.view,
            kind,
        }))
    }

    /// Releases the parked vCPU, also while the VM is paused (it continues on resume). `Continue`
    /// at a page-access event retries the access. `Emulate`/`EmulateReadData` (page-access event)
    /// and `EmulateInsn` (breakpoint event) return once the vCPU ran the instruction, every page
    /// access restriction ignored. Fails, and the vCPU stays parked untouched, on reinjection for
    /// any event but a breakpoint, a reply that does not fit the event, or an emulation the
    /// hypervisor cannot perform; fails for a vCPU that is not parked, including a second reply.
    fn reply_event(
        &mut self,
        event: Event,
        reply_type: EventReplyType,
    ) -> Result<(), Box<dyn Error>> {
        let reply = match &reply_type {
            EventReplyType::Continue { reinject, view } => Reply::Continue {
                reinject: *reinject,
                view: *view,
            },
            EventReplyType::FastSingleStep { immediate_view, next_view } => Reply::FastSingleStep {
                immediate_view: *immediate_view,
                next_view: *next_view,
            },
            EventReplyType::Emulate { view } => Reply::Emulate { view: *view },
            EventReplyType::EmulateNoWrite { view } => Reply::EmulateNoWrite { view: *view },
            EventReplyType::EmulateReadData { size, data, view } => Reply::EmulateReadData {
                data: data
                    .get(..*size as usize)
                    .ok_or(VBoxVmiDriverError::ReadDataSize(*size))?,
                view: *view,
            },
            EventReplyType::EmulateInsn { data, view } => Reply::EmulateInsn {
                data: *data,
                view: *view,
            },
        };
        Ok(self
            .client
            .get_mut()
            .reply_event(event.vcpu.into(), reply)?)
    }

    fn inject_page_fault(
        &self,
        vcpu: u16,
        virtual_address: u64,
        error_code: u32,
    ) -> Result<(), Box<dyn Error>> {
        Ok(self
            .client
            .borrow_mut()
            .inject_page_fault(vcpu.into(), virtual_address, error_code)?)
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::VirtualBox
    }
}
