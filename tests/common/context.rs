use microvmi::api::Introspectable;

pub trait Context: Send {
    fn setup(&self) -> ();
    fn init_driver(&self) -> Box<dyn Introspectable>;
    fn teardown(&self) -> ();
}

#[cfg(feature = "kvm")]
pub fn init_context() -> Box<dyn Context> {
    Box::new(super::kvm::KVM {})
}

#[cfg(all(feature = "virtualbox-api", not(feature = "kvm")))]
pub fn init_context() -> Box<dyn Context> {
    use microvmi::api::params::VBoxInitParams;
    Box::new(super::virtualbox::VirtualBox {
        method: VBoxInitParams::MachineDebugger,
    })
}

#[cfg(not(any(feature = "kvm", feature = "virtualbox-api")))]
pub fn init_context() -> Box<dyn Context> {
    panic!("Integration tests need to be run with a specific driver enabled")
}
