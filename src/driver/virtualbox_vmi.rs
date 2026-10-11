//! VirtualBox driver, VBoxVMI access method: the VBoxVMI protocol of a patched VirtualBox.
use std::error::Error;

use vboxvmi::Client;

use crate::api::{DriverType, Introspectable};

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

impl Introspectable for VBoxVmi {
    fn get_driver_type(&self) -> DriverType {
        DriverType::VirtualBox
    }
}
