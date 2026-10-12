use microvmi::api::Introspectable;

pub mod events;
pub mod get;
pub mod init;
pub mod pause;
// same condition as init_context picking the VirtualBox context
#[cfg(all(
    any(feature = "virtualbox-api", feature = "virtualbox-vmi"),
    not(feature = "kvm")
))]
pub mod virtualbox;

#[derive(Debug)]
pub struct IntegrationTest {
    pub name: &'static str,
    pub test_fn: fn(Box<dyn Introspectable>),
}

inventory::collect!(IntegrationTest);
