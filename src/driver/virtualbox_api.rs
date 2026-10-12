//! VirtualBox driver, MachineDebugger access method: VirtualBox's `IMachineDebugger` API.
use std::convert::TryFrom;
use std::error::Error;

use vboxapi::Vm;

use crate::api::params::DriverInitParams;
use crate::api::{DriverType, Introspectable, PAGE_SIZE};

#[derive(thiserror::Error, Debug)]
pub enum VBoxApiDriverError {
    #[error("VirtualBox driver requires a VM name parameter")]
    MissingVMName,
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
        let vm_name = init_params
            .common
            .ok_or(VBoxApiDriverError::MissingVMName)?
            .vm_name;
        let vm = Vm::attach(&vm_name)?;
        let max_addr = vm
            .ram_ranges()?
            .last()
            .expect("vboxapi::Vm::ram_ranges is never empty")
            .end;
        Ok(VBoxApi { vm, max_addr })
    }
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
