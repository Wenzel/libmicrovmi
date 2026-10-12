#[cfg(all(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
pub const CROSS_METHOD_TEST: &str = "cross_method_consistency";

pub fn is_selected(name: &str, filter: Option<&str>) -> bool {
    filter.is_none_or(|filter| name.contains(filter))
}
