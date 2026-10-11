//! VirtualBox driver, VBoxVMI access method: the VBoxVMI protocol of a patched VirtualBox.
use std::error::Error;

use vboxvmi::Client;

use crate::api::registers::Registers;
use crate::api::{DriverType, Introspectable};

#[derive(thiserror::Error, Debug)]
pub enum VBoxVmiDriverError {
    #[error("VirtualBox driver, VBoxVMI method: {0} is not implemented yet")]
    NotImplemented(&'static str),
}

fn not_implemented<T>(what: &'static str) -> Result<T, Box<dyn Error>> {
    Err(VBoxVmiDriverError::NotImplemented(what).into())
}

pub struct VBoxVmi {
    _client: Client,
}

impl VBoxVmi {
    /// Connects to the VM's VBoxVMI socket and performs the handshake.
    pub fn new(socket_path: &str) -> Result<Self, Box<dyn Error>> {
        Ok(VBoxVmi {
            _client: Client::connect(socket_path)?,
        })
    }
}

// shortcut: VM state access errors until the protocol has the commands (#282-#285).
impl Introspectable for VBoxVmi {
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        not_implemented("vCPU count")
    }

    fn read_physical(
        &self,
        _paddr: u64,
        _buf: &mut [u8],
        bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        *bytes_read = 0;
        not_implemented("physical memory read")
    }

    fn write_physical(&self, _paddr: u64, _buf: &[u8]) -> Result<(), Box<dyn Error>> {
        not_implemented("physical memory write")
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        not_implemented("max physical address")
    }

    fn read_registers(&self, _vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        not_implemented("register read")
    }

    fn write_registers(&self, _vcpu: u16, _reg: Registers) -> Result<(), Box<dyn Error>> {
        not_implemented("register write")
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        not_implemented("pause")
    }

    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        not_implemented("resume")
    }

    fn get_driver_type(&self) -> DriverType {
        DriverType::VirtualBox
    }
}
