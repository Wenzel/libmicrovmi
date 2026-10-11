#[cfg(feature = "kvm")]
pub static VM_NAME: &str = "winxp";
pub static VM_VCPU_COUNT: u16 = 1;
#[cfg(feature = "kvm")]
pub static VIRSH_URI: &str = "qemu:///system";
#[cfg(feature = "kvm")]
pub static KVMI_SOCKET: &str = "/tmp/introspector";
pub static TIMEOUT: u64 = 20;
