use std::time::Duration;

use microvmi::api::Introspectable;

use super::config::{TIMEOUT, VM_VCPU_COUNT};

pub trait Context: Send {
    fn setup(&self);
    fn init_driver(&self) -> Box<dyn Introspectable>;
    fn teardown(&self);
    /// Per-test timeout, after which the remaining tests are aborted.
    fn timeout(&self) -> Duration {
        Duration::from_secs(TIMEOUT)
    }
    /// The VM's vCPU count, which the driver must report.
    fn vcpu_count(&self) -> u16 {
        VM_VCPU_COUNT
    }
}

#[cfg(feature = "kvm")]
pub fn init_context() -> Box<dyn Context> {
    Box::new(super::kvm::KvmContext {})
}

#[cfg(all(
    any(feature = "virtualbox-api", feature = "virtualbox-vmi"),
    not(feature = "kvm")
))]
pub fn init_context() -> Box<dyn Context> {
    Box::new(super::virtualbox::VirtualBox::from_env())
}

#[cfg(not(any(
    feature = "kvm",
    feature = "virtualbox-api",
    feature = "virtualbox-vmi"
)))]
pub fn init_context() -> Box<dyn Context> {
    panic!("Integration tests need to be run with a specific driver enabled")
}
