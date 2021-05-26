use microvmi::api::Introspectable;

pub mod init;

#[derive(Debug)]
pub struct IntegrationTest {
    pub name: &'static str,
    pub test_fn: fn(Box<dyn Introspectable>),
}

inventory::collect!(IntegrationTest);
