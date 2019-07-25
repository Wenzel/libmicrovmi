pub mod dummy;
#[cfg(feature="xen")]
pub mod xen;
#[cfg(feature="kvm")]
pub mod kvm;
