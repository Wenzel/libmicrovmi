use super::IntegrationTest;
use crate::common::config::VM_VCPU_COUNT;
use microvmi::api::Introspectable;

fn get_vcpu_count(drv: Box<dyn Introspectable>) {
    assert_eq!(VM_VCPU_COUNT, drv.get_vcpu_count().unwrap());
}

inventory::submit!(IntegrationTest {
    name: "get_vcpu_count",
    test_fn: get_vcpu_count
});
