use enum_iterator::IntoEnumIterator;
use std::error::Error;

use crate::errors::MicrovmiError;
use events::{Event, EventReplyType, InterceptType};
use registers::Registers;

pub mod events;
pub mod params;
pub mod registers;

#[allow(clippy::bad_bit_mask)]
mod flags {
    bitflags! {
        #[repr(transparent)]
        pub struct Access: u32 {
            const NIL=0b00000000;
            const R=0b00000001;
            const W=0b00000010;
            const X=0b00000100;
            const RW=Self::R.bits | Self::W.bits;
            const WX=Self::W.bits | Self::X.bits;
            const RX=Self::R.bits | Self::X.bits;
            const RWX=Self::R.bits | Self::W.bits | Self::X.bits;
        }
    }
}
pub use flags::Access;

///Represents the available hypervisor VMI drivers supported by libmicrovmi
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, IntoEnumIterator)]
pub enum DriverType {
    KVM,
    Memflow,
    VirtualBox,
    Xen,
}

// impl TryInto<DriverInitParam> for DriverInitParamFFI {
//     type Error = IntoStringError;
//
//     fn try_into(self) -> Result<DriverInitParam, Self::Error> {
//         Ok(match self {
//             DriverInitParamFFI::KVMiSocket(cstr_socket) => DriverInitParam::KVMiSocket(
//                 unsafe { CStr::from_ptr(cstr_socket) }
//                     .to_owned()
//                     .into_string()?,
//             ),
//         })
//     }
// }

pub const PAGE_SHIFT: u32 = 12;
pub const PAGE_SIZE: u32 = 4096;

/// The Default view.
pub const DEFAULT_MEMORY_VIEW: u16 = 0;
/// Keep the parked vCPU's Active view in an event reply.
pub const KEEP_MEMORY_VIEW: u16 = 0xffff;
/// Restore a Remap's source page.
pub const RESTORE_PAGE: u64 = 0xffff_ffff_ffff_ffff;

/// The result of an `Introspectable` method a driver does not implement.
fn not_supported<T>(operation: &str) -> Result<T, Box<dyn Error>> {
    Err(MicrovmiError::NotSupported(operation.into()).into())
}

/// Every method but `get_driver_type` defaults to a [`MicrovmiError::NotSupported`] error.
pub trait Introspectable {
    /// Retrieve the number of VCPUs.
    ///
    fn get_vcpu_count(&self) -> Result<u16, Box<dyn Error>> {
        not_supported("get_vcpu_count")
    }

    /// read the physical memory, starting from paddr, into buf
    ///
    /// # Arguments
    ///
    /// * 'paddr' - the physical address to read from
    /// * 'buf' - the data read from memory
    /// * 'bytes_read' - the number of bytes read
    ///
    fn read_physical(
        &self,
        _paddr: u64,
        _buf: &mut [u8],
        _bytes_read: &mut u64,
    ) -> Result<(), Box<dyn Error>> {
        not_supported("read_physical")
    }

    /// Modify contents of physical memory, starting at paddr, from buf
    ///
    /// # Arguments
    ///
    /// * 'paddr' - the physical address to write into
    /// * 'buf' - the data to be written into memory
    ///
    fn write_physical(&self, _paddr: u64, _buf: &[u8]) -> Result<(), Box<dyn Error>> {
        not_supported("write_physical")
    }

    /// Get the maximum physical address
    ///
    /// Returns maximum physical address in 64 bit unsigned integer format.
    ///
    fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        not_supported("get_max_physical_addr")
    }

    /// Read register values
    ///
    /// # Arguments
    /// * 'vcpu' - vcpu id for which the value of registers are to be dumped as the argument
    ///
    fn read_registers(&self, _vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        not_supported("read_registers")
    }

    /// The allowed access of `paddr`'s page in `view`.
    fn get_page_access(&self, _view: u16, _paddr: u64) -> Result<Access, Box<dyn Error>> {
        not_supported("get_page_access")
    }

    /// Sets the allowed access of `paddr`'s page in `view`.
    fn set_page_access(
        &self,
        _view: u16,
        _paddr: u64,
        _access: Access,
    ) -> Result<(), Box<dyn Error>> {
        not_supported("set_page_access")
    }

    /// Enables or disables Memory views.
    fn enable_memory_views(&self, _enabled: bool) -> Result<(), Box<dyn Error>> {
        not_supported("enable_memory_views")
    }

    /// Creates a Memory view and returns its view number.
    fn create_memory_view(&self) -> Result<u16, Box<dyn Error>> {
        not_supported("create_memory_view")
    }

    /// Destroys a Memory view.
    fn destroy_memory_view(&self, _view: u16) -> Result<(), Box<dyn Error>> {
        not_supported("destroy_memory_view")
    }

    /// Switches every vCPU to a Memory view.
    fn switch_memory_view(&self, _view: u16) -> Result<(), Box<dyn Error>> {
        not_supported("switch_memory_view")
    }

    /// Remaps `page` to `target` in `view`; [`RESTORE_PAGE`] restores the source page.
    fn remap_memory_page(
        &self,
        _view: u16,
        _page: u64,
        _target: u64,
    ) -> Result<(), Box<dyn Error>> {
        not_supported("remap_memory_page")
    }

    /// Allocates a Hidden page and returns its guest-physical page number.
    fn allocate_hidden_page(&self) -> Result<u64, Box<dyn Error>> {
        not_supported("allocate_hidden_page")
    }

    /// Returns a Hidden page to the pool.
    fn free_hidden_page(&self, _page: u64) -> Result<(), Box<dyn Error>> {
        not_supported("free_hidden_page")
    }

    /// Returns the Hidden page pool size (VBoxInternal/VMI/HiddenPages), or 0 if not applicable.
    fn hidden_page_pool_size(&self) -> u32 {
        0
    }

    /// Write register values
    ///
    /// # Arguments
    /// * 'vcpu' - vcpu id for which the value of registers are to be set
    /// * 'reg' - Registers enum having values to be set
    ///
    fn write_registers(&self, _vcpu: u16, _reg: Registers) -> Result<(), Box<dyn Error>> {
        not_supported("write_registers")
    }

    /// Used to pause the VM
    ///
    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        not_supported("pause")
    }

    /// Used to resume the VM
    ///
    fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        not_supported("resume")
    }

    /// Used to enable/disable an event interception
    ///
    /// # Arguments
    /// * 'vcpu' - vcpu id for which we are to enable/disable intercept monitoring
    /// * 'intercept_type' - to specify event type for which to raise flag
    /// * 'enabled' - flag to specify whether to enable/disable event monitoring
    ///
    fn toggle_intercept(
        &mut self,
        _vcpu: u16,
        _intercept_type: InterceptType,
        _enabled: bool,
    ) -> Result<(), Box<dyn Error>> {
        not_supported("toggle_intercept")
    }

    /// Listen and return the next event, or None
    ///
    /// # Arguments
    /// * 'timeout' - Time in milliseconds for which it will wait for a new event
    ///
    fn listen(&mut self, _timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        not_supported("listen")
    }

    /// Send reply corresponding to the current event being popped
    ///
    /// Consumes the event: replying to it again is not possible.
    ///
    /// # Arguments
    /// * 'event' - the event returned by `listen`
    /// * 'reply_type' - how the vCPU resumes, see [`EventReplyType`]
    ///
    fn reply_event(
        &mut self,
        _event: Event,
        _reply_type: EventReplyType,
    ) -> Result<(), Box<dyn Error>> {
        not_supported("reply_event")
    }

    /// Queues an x86 #PF for the parked `vcpu`, before `reply_event`. The vCPU stays parked;
    /// `virtual_address` becomes CR2 and `error_code` is the x86 page-fault error code.
    fn inject_page_fault(
        &self,
        _vcpu: u16,
        _virtual_address: u64,
        _error_code: u32,
    ) -> Result<(), Box<dyn Error>> {
        not_supported("inject_page_fault")
    }

    /// Return the concrete DriverType
    fn get_driver_type(&self) -> DriverType;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::events::{CrType, EventType};
    use crate::api::registers::X86Registers;

    struct Nothing;

    impl Introspectable for Nothing {
        fn get_driver_type(&self) -> DriverType {
            DriverType::KVM
        }
    }

    #[test]
    fn defaults_return_not_supported() {
        let mut d = Nothing;
        assert!(d.get_vcpu_count().is_err());
        assert!(d.read_physical(0, &mut [0; 1], &mut 0).is_err());
        assert!(d.write_physical(0, &[0]).is_err());
        assert!(d.get_max_physical_addr().is_err());
        assert!(d.read_registers(0).is_err());
        assert!(d.get_page_access(DEFAULT_MEMORY_VIEW, 0).is_err());
        assert!(d
            .set_page_access(DEFAULT_MEMORY_VIEW, 0, Access::R)
            .is_err());
        assert!(d.enable_memory_views(true).is_err());
        assert!(d.create_memory_view().is_err());
        assert!(d.destroy_memory_view(1).is_err());
        assert!(d.switch_memory_view(1).is_err());
        assert!(d.remap_memory_page(1, 0, 1).is_err());
        assert!(d.allocate_hidden_page().is_err());
        assert!(d.free_hidden_page(0).is_err());
        assert!(d
            .write_registers(0, Registers::X86(X86Registers::default()))
            .is_err());
        assert!(d.pause().is_err());
        assert!(d.resume().is_err());
        assert!(d
            .toggle_intercept(0, InterceptType::Cr(CrType::Cr3), true)
            .is_err());
        assert!(d.listen(0).is_err());
        let event = Event {
            vcpu: 0,
            view: DEFAULT_MEMORY_VIEW,
            kind: EventType::Singlestep { gpa: 0 },
        };
        assert!(d
            .reply_event(
                event,
                EventReplyType::Continue {
                    reinject: false,
                    view: KEEP_MEMORY_VIEW,
                },
            )
            .is_err());
        let e = d.inject_page_fault(0, 0x1234, 0).unwrap_err().to_string();
        assert!(e.contains("inject_page_fault") && e.contains("not supported"), "{}", e);
        let e = d.pause().unwrap_err().to_string();
        assert!(e.contains("pause") && e.contains("not supported"), "{}", e);
    }
}
