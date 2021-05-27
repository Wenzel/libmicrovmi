use std::process::{Command, Stdio};

use log::debug;
use microvmi::api::{DriverInitParam, DriverType, Introspectable};
use microvmi::init;

use super::config::{KVMI_SOCKET, VIRSH_URI, VM_NAME};
use crate::common::context::Context;

pub struct KVM;

impl Context for KVM {
    /// restore VM state from internal QEMU snapshot
    fn setup(&self) {
        debug!("setup test");
        Command::new("virsh")
            .arg(format!("--connect={}", VIRSH_URI))
            .arg("snapshot-revert")
            .arg(VM_NAME)
            .arg("--current")
            .arg("--running")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("Failed to start virsh")
            .success()
            .then(|| 0)
            .expect("Failed to run virsh snapshot-revert");
    }

    fn init_driver(&self) -> Box<dyn Introspectable> {
        init(
            VM_NAME,
            Some(DriverType::KVM),
            Some(DriverInitParam::KVMiSocket(String::from(KVMI_SOCKET))),
        )
        .expect("Failed to init libmicrovmi")
    }

    /// shutdown VM
    fn teardown(&self) {
        debug!("teardown test");
        Command::new("virsh")
            .arg(format!("--connect={}", VIRSH_URI))
            .arg("destroy")
            .arg(VM_NAME)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("Failed to start virsh")
            .success()
            .then(|| 0)
            .expect("Failed to run virsh destroy");
    }
}
