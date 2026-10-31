#[cfg(all(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
pub const CROSS_METHOD_TEST: &str = "cross_method_consistency";

pub fn is_selected(name: &str, filter: Option<&str>) -> bool {
    // Guest-SSH tests use a separate Ubuntu VM; the default Alpine suite cannot run them.
    if filter.is_none() && name.starts_with("vboxvmi_ssh_") {
        return false;
    }
    filter.is_none_or(|filter| name.contains(filter))
}
