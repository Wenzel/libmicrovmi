pub mod dummy;
#[cfg(feature = "kvm")]
pub mod kvm;
#[cfg(feature = "xen")]
pub mod xen;
#[cfg(feature = "virtualbox")]
pub mod virtualbox;