use microvmi::api::events::{CrType, EventReplyType, EventType, InterceptType};
use microvmi::api::Introspectable;

use super::IntegrationTest;

fn intercept_cr3_one(mut drv: Box<dyn Introspectable>) {
    for vcpu in 0..drv.get_vcpu_count().unwrap() - 1 {
        drv.toggle_intercept(vcpu, InterceptType::Cr(CrType::Cr3), true)
            .expect("Failed to toggle CR3 intercept");
    }
    let event = drv.listen(5000).unwrap().unwrap();
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
    for vcpu in 0..drv.get_vcpu_count().unwrap() - 1 {
        drv.toggle_intercept(vcpu, InterceptType::Cr(CrType::Cr3), true)
            .expect("Failed to toggle CR3 intercept");
    }
    for _ in 0..10 {
        let event = drv.listen(5000).unwrap().unwrap();
        match event.kind {
            EventType::Cr { cr_type, .. } => {
                assert_eq!(cr_type, CrType::Cr3);
                drv.reply_event(event, EventReplyType::Continue)
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
