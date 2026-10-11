use super::IntegrationTest;
use crate::common::context::init_context;
use microvmi::api::Introspectable;

fn get_vcpu_count(drv: Box<dyn Introspectable>) {
    assert_eq!(init_context().vcpu_count(), drv.get_vcpu_count().unwrap());
}

inventory::submit!(IntegrationTest {
    name: "get_vcpu_count",
    test_fn: get_vcpu_count
});
