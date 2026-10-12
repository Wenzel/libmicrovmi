//! VirtualBox driver, VBoxVMI access method: the VBoxVMI protocol of a patched VirtualBox.
use std::cell::RefCell;
use std::convert::TryFrom;
use std::error::Error;
use std::path::Path;

use vboxvmi::{Client, Segment, TableRegister};

use crate::api::registers::{Registers, SegmentReg, SystemTableReg, X86Registers};
use crate::api::{DriverType, Introspectable};
use crate::driver::EferMismatch;

#[derive(thiserror::Error, Debug)]
pub enum VBoxVmiDriverError {
    #[error("VirtualBox driver, VBoxVMI method: guest physical page {addr:#x} is unreadable (VirtualBox status {rc})")]
    Unreadable { addr: u64, rc: i32 },
    #[error("VirtualBox driver, VBoxVMI method: guest physical page {addr:#x} is unwritable (VirtualBox status {rc})")]
    Unwritable { addr: u64, rc: i32 },
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

    fn get_driver_type(&self) -> DriverType {
        DriverType::VirtualBox
    }
}
