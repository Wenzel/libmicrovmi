use super::IntegrationTest;
use microvmi::api::Introspectable;

fn test_init(_drv: Box<dyn Introspectable>) {
    // nothing to do
}

inventory::submit!(IntegrationTest {
    name: "test_init",
    test_fn: test_init
});
