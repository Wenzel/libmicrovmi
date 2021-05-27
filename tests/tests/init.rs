use super::IntegrationTest;
use microvmi::api::Introspectable;

fn init(_drv: Box<dyn Introspectable>) {
    // nothing to do
}

inventory::submit!(IntegrationTest {
    name: "init",
    test_fn: init
});
