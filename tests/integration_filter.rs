#![cfg(all(feature = "virtualbox-api", feature = "virtualbox-vmi"))]

#[path = "common/filter.rs"]
mod filter;

use filter::{is_selected, CROSS_METHOD_TEST};

#[test]
fn cross_method_requires_its_exact_filter() {
    assert!(!is_selected(CROSS_METHOD_TEST, Some("virtualbox_")));
    assert!(is_selected(CROSS_METHOD_TEST, Some(CROSS_METHOD_TEST)));
}
