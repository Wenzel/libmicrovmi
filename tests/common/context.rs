use super::kvm::KVM;
use microvmi::api::Introspectable;

pub trait Context: Send {
    fn setup(&self) -> ();
    fn init_driver(&self) -> Box<dyn Introspectable>;
    fn teardown(&self) -> ();
}

pub fn init_context() -> Box<dyn Context> {
    if cfg!(feature = "kvm") {
        Box::new(KVM {})
    } else {
        panic!("Integration tests need to be run with a specific driver enabled")
    }
}
