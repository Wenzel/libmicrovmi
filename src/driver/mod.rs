#[cfg(feature = "kvm")]
pub mod kvm;
#[cfg(feature = "mflow")]
pub mod memflow;
#[cfg(feature = "virtualbox")]
pub mod virtualbox;
#[cfg(feature = "virtualbox-api")]
pub mod virtualbox_api;
#[cfg(feature = "virtualbox-vmi")]
pub mod virtualbox_vmi;
#[cfg(feature = "xen")]
pub mod xen;

/// VirtualBox has one EFER register, which `X86Registers::efer` and `msr_efer` both are: a register
/// write giving them different values is refused by both VirtualBox access methods.
#[cfg(any(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
#[derive(thiserror::Error, Debug)]
#[error(
    "VirtualBox driver: efer {efer:#x} and msr_efer {msr_efer:#x} are both EFER and must be equal"
)]
pub struct EferMismatch {
    pub efer: u64,
    pub msr_efer: u64,
}

#[cfg(any(feature = "virtualbox-api", feature = "virtualbox-vmi"))]
impl EferMismatch {
    /// Checks a register set to be written.
    fn check(r: &crate::api::registers::X86Registers) -> Result<(), Self> {
        if r.efer == r.msr_efer {
            return Ok(());
        }
        Err(EferMismatch {
            efer: r.efer,
            msr_efer: r.msr_efer,
        })
    }
}
