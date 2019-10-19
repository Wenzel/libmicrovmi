pub mod dummy;
#[cfg(feature="xen")]
pub mod xen;
#[cfg(feature="kvm")]
pub mod kvm;
#[cfg(feature="hyper-v")]
pub mod hyperv;
