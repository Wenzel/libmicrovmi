//! VirtualBox driver, MachineDebugger access method: VirtualBox's `IMachineDebugger` API.
use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;
#[cfg(feature = "virtualbox-vmi")]
use std::path::PathBuf;

use vboxapi::Vm;

use crate::api::params::DriverInitParams;
use crate::api::registers::{Registers, SegmentReg, SystemTableReg, X86Registers};
use crate::api::{DriverType, Introspectable, PAGE_SIZE};
use crate::driver::EferMismatch;

#[derive(thiserror::Error, Debug)]
pub enum VBoxApiDriverError {
    #[error("VirtualBox driver requires a VM name parameter")]
    MissingVMName,
    #[error("no registers for vCPU {0}")]
    NoSuchVcpu(u16),
    #[error("register {name}: missing or unparseable value {value:?}")]
    BadRegister {
        name: &'static str,
        value: Option<String>,
    },
}

/// `X86Registers` fields as DBGF register names (`IMachineDebugger` register names) and values.
/// Segment selectors are the bare segment names (`cs`). `efer` is left out: it must equal
/// `msr_efer`, the same DBGF register.
fn dbgf_registers(r: &X86Registers) -> Vec<(&'static str, u64)> {
    let mut regs = vec![
        ("rax", r.rax),
        ("rbx", r.rbx),
        ("rcx", r.rcx),
        ("rdx", r.rdx),
        ("rsi", r.rsi),
        ("rdi", r.rdi),
        ("rsp", r.rsp),
        ("rbp", r.rbp),
        ("r8", r.r8),
        ("r9", r.r9),
        ("r10", r.r10),
        ("r11", r.r11),
        ("r12", r.r12),
        ("r13", r.r13),
        ("r14", r.r14),
        ("r15", r.r15),
        ("rip", r.rip),
        ("rflags", r.rflags),
        ("cr0", r.cr0),
        ("cr2", r.cr2),
        ("cr3", r.cr3),
        ("cr4", r.cr4),
        ("sysenter_cs", r.sysenter_cs),
        ("sysenter_esp", r.sysenter_esp),
        ("sysenter_eip", r.sysenter_eip),
        ("efer", r.msr_efer),
        ("star", r.msr_star),
        ("lstar", r.msr_lstar),
        ("apic_base", r.apic_base),
        ("gdtr_base", r.gdt.base),
        ("gdtr_lim", r.gdt.limit.into()),
        ("idtr_base", r.idt.base),
        ("idtr_lim", r.idt.limit.into()),
    ];
    for (name, seg) in SEGMENTS.iter().zip(segments(r)) {
        regs.extend([
            (name[0], seg.selector.into()),
            (name[1], seg.base),
            (name[2], seg.limit.into()),
        ]);
    }
    regs
}

/// DBGF names of each segment register's selector, base and limit, in `segments` order.
const SEGMENTS: [[&str; 3]; 8] = [
    ["cs", "cs_base", "cs_lim"],
    ["ds", "ds_base", "ds_lim"],
    ["es", "es_base", "es_lim"],
    ["fs", "fs_base", "fs_lim"],
    ["gs", "gs_base", "gs_lim"],
    ["ss", "ss_base", "ss_lim"],
    ["tr", "tr_base", "tr_lim"],
    ["ldtr", "ldtr_base", "ldtr_lim"],
];

fn segments(r: &X86Registers) -> [&SegmentReg; 8] {
    [&r.cs, &r.ds, &r.es, &r.fs, &r.gs, &r.ss, &r.tr, &r.ldt]
}

/// Not `Send`: the VirtualBox session must stay on the thread that opened it, which
/// `Box<dyn Introspectable>` (not `Send` either) already guarantees.
pub struct VBoxApi {
    vm: Vm,
    /// End of the highest RAM range, fixed for the VM's lifetime.
    max_addr: u64,
}

impl VBoxApi {
    pub fn new(init_params: DriverInitParams) -> Result<Self, Box<dyn Error>> {
        let vm = Vm::attach(vm_name(&init_params)?)?;
        let max_addr = vm
            .ram_ranges()?
            .last()
            .expect("vboxapi::Vm::ram_ranges is never empty")
            .end;
        Ok(VBoxApi { vm, max_addr })
    }
}

fn vm_name(init_params: &DriverInitParams) -> Result<&str, VBoxApiDriverError> {
    Ok(&init_params
        .common
        .as_ref()
        .ok_or(VBoxApiDriverError::MissingVMName)?
        .vm_name)
}

/// The VM's default VBoxVMI socket, resolved through the VirtualBox API.
#[cfg(feature = "virtualbox-vmi")]
pub fn vmi_socket_path(init_params: &DriverInitParams) -> Result<PathBuf, Box<dyn Error>> {
    Ok(vboxapi::vmi_socket_path(vm_name(init_params)?)?)
}

impl Introspectable for VBoxApi {
    /// On error, `bytes_read` is the length of the readable prefix, up to the first unreadable page.
    fn read_physical(
        &self,
        paddr: u64,
        buf: &mut [u8],
        bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        *bytes_read = 0;
        if self.vm.read_physical(paddr, buf).is_ok() {
            *bytes_read = buf.len() as u64;
            return Ok(());
        }
        // Reads are all-or-nothing: retry page by page to find the readable prefix.
        let mut offset = 0;
        while offset < buf.len() {
            let page_size = u64::from(PAGE_SIZE);
            let page_left = (page_size - (paddr + offset as u64) % page_size) as usize;
            let end = buf.len().min(offset + page_left);
            self.vm
                .read_physical(paddr + offset as u64, &mut buf[offset..end])?;
            offset = end;
            *bytes_read = offset as u64;
        }
        Ok(())
    }

    /// Not atomic: on error, a prefix of `buf` may already be written, up to the first page that is
    /// not backed by host memory (MMIO, ballooned, unassigned), which is never written.
    fn write_physical(&self, paddr: u64, buf: &[u8]) -> Result<(), Box<dyn Error>> {
        Ok(self.vm.write_physical(paddr, buf)?)
    }

    /// One `getRegisters` call. The MSRs `X86Registers` has (EFER, STAR, LSTAR, SYSENTER_*,
    /// APIC base) are all read; `efer` and `msr_efer` are both EFER.
    fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        let prefix = format!("cpu{}.", vcpu);
        let values: HashMap<String, String> = self
            .vm
            .registers(vcpu.into())?
            .into_iter()
            .filter_map(|(name, value)| Some((name.strip_prefix(&prefix)?.to_string(), value)))
            .collect();
        if values.is_empty() {
            return Err(VBoxApiDriverError::NoSuchVcpu(vcpu).into());
        }
        // integers are formatted as 0x-prefixed hex
        let get = |name: &'static str| -> Result<u64, VBoxApiDriverError> {
            let value = values.get(name);
            value
                .and_then(|v| u64::from_str_radix(v.strip_prefix("0x")?, 16).ok())
                .ok_or_else(|| VBoxApiDriverError::BadRegister {
                    name,
                    value: value.cloned(),
                })
        };
        // values come from registers of that width
        let seg =
            |[selector, base, limit]: [&'static str; 3]| -> Result<SegmentReg, VBoxApiDriverError> {
                Ok(SegmentReg {
                    base: get(base)?,
                    limit: get(limit)? as u32,
                    selector: get(selector)? as u16,
                })
            };
        let [cs, ds, es, fs, gs, ss, tr, ldt] = SEGMENTS;
        Ok(Registers::X86(X86Registers {
            rax: get("rax")?,
            rbx: get("rbx")?,
            rcx: get("rcx")?,
            rdx: get("rdx")?,
            rsi: get("rsi")?,
            rdi: get("rdi")?,
            rsp: get("rsp")?,
            rbp: get("rbp")?,
            r8: get("r8")?,
            r9: get("r9")?,
            r10: get("r10")?,
            r11: get("r11")?,
            r12: get("r12")?,
            r13: get("r13")?,
            r14: get("r14")?,
            r15: get("r15")?,
            rip: get("rip")?,
            rflags: get("rflags")?,
            cr0: get("cr0")?,
            cr2: get("cr2")?,
            cr3: get("cr3")?,
            cr4: get("cr4")?,
            sysenter_cs: get("sysenter_cs")?,
            sysenter_esp: get("sysenter_esp")?,
            sysenter_eip: get("sysenter_eip")?,
            msr_efer: get("efer")?,
            msr_star: get("star")?,
            msr_lstar: get("lstar")?,
            efer: get("efer")?,
            apic_base: get("apic_base")?,
            cs: seg(cs)?,
            ds: seg(ds)?,
            es: seg(es)?,
            fs: seg(fs)?,
            gs: seg(gs)?,
            ss: seg(ss)?,
            tr: seg(tr)?,
            ldt: seg(ldt)?,
            idt: SystemTableReg {
                base: get("idtr_base")?,
                limit: get("idtr_lim")? as u16,
            },
            gdt: SystemTableReg {
                base: get("gdtr_base")?,
                limit: get("gdtr_lim")? as u16,
            },
        }))
    }

    /// Writes only the registers that differ from the current ones, in one `setRegisters` call,
    /// so unchanged control registers and MSRs are not reloaded. Segment selectors, bases and
    /// limits are written as raw debugger state. Pause the VM first: on a running VM the
    /// registers are not written all at once. Not atomic: on error, the registers before the
    /// failing one stay written. `efer` and `msr_efer` are both EFER: they must be equal.
    fn write_registers(&self, vcpu: u16, reg: Registers) -> Result<(), Box<dyn Error>> {
        let Registers::X86(new) = reg;
        EferMismatch::check(&new)?;
        let Registers::X86(old) = self.read_registers(vcpu)?;
        let mut changed = Vec::new();
        for ((name, value), (_, old_value)) in
            dbgf_registers(&new).into_iter().zip(dbgf_registers(&old))
        {
            if value == old_value {
                continue;
            }
            changed.push((name, format!("{:#x}", value)));
        }
        if changed.is_empty() {
            return Ok(());
        }
        let changed: Vec<(&str, &str)> = changed.iter().map(|(n, v)| (*n, v.as_str())).collect();
        Ok(self.vm.set_registers(vcpu.into(), &changed)?)
    }

    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        Ok(u16::try_from(self.vm.cpu_count())?)
    }

    /// Synchronous and idempotent: one `resume` undoes any number of `pause`s.
    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(self.vm.pause()?)
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(self.vm.resume()?)
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        Ok(self.max_addr)
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::VirtualBox
    }
}
