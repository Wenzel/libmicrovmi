use std::process::{Command, Stdio};

use log::debug;

use crate::common::config::{KVMI_SOCKET, VIRSH_URI, VM_NAME};
use microvmi::api::{DriverInitParam, DriverType, Introspectable};
use microvmi::init;

/// restore VM state from internal QEMU snapshot
pub fn setup_test() {
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

/// shutdown VM
pub fn teardown_test() {
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

pub fn init_driver() -> Box<dyn Introspectable> {
    init(
        VM_NAME,
        Some(DriverType::KVM),
        Some(DriverInitParam::KVMiSocket(String::from(KVMI_SOCKET))),
    )
    .expect("Failed to init libmicrovmi")
}
