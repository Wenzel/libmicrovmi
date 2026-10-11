//! VirtualBox driver, MachineDebugger access method: VirtualBox's `IMachineDebugger` API.
use std::error::Error;

use vboxapi::Vm;

use crate::api::{DriverType, Introspectable};

/// Not `Send`: the VirtualBox session must stay on the thread that opened it, which
/// `Box<dyn Introspectable>` (not `Send` either) already guarantees.
pub struct VBoxApi {
    vm: Vm,
    /// End of the highest RAM range, fixed for the VM's lifetime.
    max_addr: u64,
}

impl VBoxApi {
    pub fn new(vm_name: &str) -> Result<Self, Box<dyn Error>> {
        let vm = Vm::attach(vm_name)?;
        let max_addr = vm.ram_ranges()?.last().map_or(0, |r| r.end);
        Ok(VBoxApi { vm, max_addr })
    }
}

impl Introspectable for VBoxApi {
    fn read_physical(
        &self,
        paddr: u64,
        buf: &mut [u8],
        bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        *bytes_read = 0;
        self.vm.read_physical(paddr, buf)?;
        *bytes_read = buf.len() as u64;
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        Ok(self.max_addr)
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::VirtualBox
    }
}
