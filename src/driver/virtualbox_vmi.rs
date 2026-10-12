//! VirtualBox driver, VBoxVMI access method: the VBoxVMI protocol of a patched VirtualBox.
use std::cell::RefCell;
use std::convert::TryFrom;
use std::error::Error;

use vboxvmi::Client;

use crate::api::registers::Registers;
use crate::api::{DriverType, Introspectable};

#[derive(thiserror::Error, Debug)]
pub enum VBoxVmiDriverError {
    #[error("VirtualBox driver, VBoxVMI method: {0} is not implemented yet")]
    NotImplemented(&'static str),
    #[error("VirtualBox driver, VBoxVMI method: guest physical page {addr:#x} is unreadable (VirtualBox status {rc})")]
    Unreadable { addr: u64, rc: i32 },
}

fn not_implemented<T>(what: &'static str) -> Result<T, Box<dyn Error>> {
    Err(VBoxVmiDriverError::NotImplemented(what).into())
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
    pub fn new(socket_path: &str) -> Result<Self, Box<dyn Error>> {
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

// shortcut: VM state access errors until the protocol has the commands (#284-#285).
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

    fn write_physical(&self, _paddr: u64, _buf: &[u8]) -> Result<(), Box<dyn Error>> {
        not_implemented("physical memory write")
    }

    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        Ok(self.max_addr)
    }

    fn read_registers(&self, _vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        not_implemented("register read")
    }

    fn write_registers(&self, _vcpu: u16, _reg: Registers) -> Result<(), Box<dyn Error>> {
        not_implemented("register write")
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
