use microvmi::api::events::{CrType, EventReplyType, EventType, InterceptType};
use microvmi::api::Introspectable;

use super::{next_event, IntegrationTest};
use crate::common::context::init_context;

fn intercept_cr3_one(mut drv: Box<dyn Introspectable>) {
    let ctx = init_context();
    for vcpu in 0..drv.get_vcpu_count().unwrap() {
        drv.toggle_intercept(vcpu, InterceptType::Cr(CrType::Cr3), true)
            .expect("Failed to toggle CR3 intercept");
    }
    let event = next_event(drv.as_mut(), &mut || ctx.start_guest_process());
    assert!(matches!(
        event.kind,
        EventType::Cr {
            cr_type: CrType::Cr3,
            ..
        }
    ));
}

inventory::submit!(IntegrationTest {
    name: "intercept_cr3_one",
    test_fn: intercept_cr3_one
});

fn intercept_cr3_multiple(mut drv: Box<dyn Introspectable>) {
    let ctx = init_context();
    for vcpu in 0..drv.get_vcpu_count().unwrap() {
        drv.toggle_intercept(vcpu, InterceptType::Cr(CrType::Cr3), true)
            .expect("Failed to toggle CR3 intercept");
    }
    for _ in 0..10 {
        let event = next_event(drv.as_mut(), &mut || ctx.start_guest_process());
        match event.kind {
            EventType::Cr { cr_type, .. } => {
                assert_eq!(cr_type, CrType::Cr3);
                drv.reply_event(event, EventReplyType::Continue { reinject: false })
                    .expect("Failed to send event reply");
            }
            _ => panic!("Failed"),
        }
    }
}

inventory::submit!(IntegrationTest {
    name: "intercept_cr3_multiple",
    test_fn: intercept_cr3_multiple
});
