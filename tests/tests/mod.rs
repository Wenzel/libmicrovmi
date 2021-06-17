use microvmi::api::Introspectable;

pub mod events;
pub mod get;
pub mod init;
pub mod pause;

#[derive(Debug)]
pub struct IntegrationTest {
    pub name: &'static str,
    pub test_fn: fn(Box<dyn Introspectable>),
}

inventory::collect!(IntegrationTest);
