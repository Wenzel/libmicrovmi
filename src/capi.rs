use crate::api::{DriverType, Introspectable, Registers};
use crate::driver::dummy::Dummy;
#[cfg(feature = "hyper-v")]
use crate::driver::hyperv::HyperV;
#[cfg(feature = "kvm")]
use crate::driver::kvm::Kvm;
#[cfg(feature = "virtualbox")]
use crate::driver::virtualbox::VBox;
#[cfg(feature = "xen")]
use crate::driver::xen::Xen;
use crate::init;
use cty::{c_char, size_t, uint16_t, uint64_t, uint8_t};
use std::ffi::{c_void, CStr};
use std::slice;

#[repr(C)]
pub struct MicrovmiContext {
    driver: *mut c_void,
    driver_type: DriverType,
}

#[repr(C)]
pub enum MicrovmiStatus {
    MicrovmiSuccess,
    MicrovmiFailure,
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_init(
    domain_name: *const c_char,
    driver_type: DriverType,
) -> *mut MicrovmiContext {
    let safe_domain_name = CStr::from_ptr(domain_name).to_string_lossy().into_owned();
    let driver = init(&safe_domain_name, Some(driver_type.clone()));
    Box::into_raw(Box::new(MicrovmiContext {
        driver: Box::into_raw(driver) as *mut c_void,
        driver_type,
    }))
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_destroy(context: *mut MicrovmiContext) {
    let boxed_context = Box::from_raw(context);
    let _ = get_driver_box(&boxed_context);
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_pause(context: *mut MicrovmiContext) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context.as_ref().unwrap());
    match (*driver).pause() {
        Ok(_) => MicrovmiStatus::MicrovmiSuccess,
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_resume(context: *mut MicrovmiContext) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context.as_ref().unwrap());
    match (*driver).resume() {
        Ok(_) => MicrovmiStatus::MicrovmiSuccess,
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_read_physical(
    context: *mut MicrovmiContext,
    physical_address: uint64_t,
    buffer: *mut uint8_t,
    size: size_t,
) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context.as_ref().unwrap());
    match (*driver).read_physical(physical_address, slice::from_raw_parts_mut(buffer, size)) {
        Ok(_) => MicrovmiStatus::MicrovmiSuccess,
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_get_max_physical_addr(
    context: *mut MicrovmiContext,
    address_ptr: *mut uint64_t,
) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context.as_ref().unwrap());
    match (*driver).get_max_physical_addr() {
        Ok(max_addr) => {
            address_ptr.write(max_addr);
            MicrovmiStatus::MicrovmiSuccess
        }
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_read_registers(
    context: *mut MicrovmiContext,
    vcpu: uint16_t,
    registers: *mut Registers,
) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context.as_ref().unwrap());
    match (*driver).read_registers(vcpu) {
        Ok(regs) => {
            registers.write(regs);
            MicrovmiStatus::MicrovmiSuccess
        }
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

unsafe fn get_driver_mut_ptr(context: &MicrovmiContext) -> *mut dyn Introspectable {
    match context.driver_type {
        DriverType::Dummy => context.driver as *mut Dummy as *mut dyn Introspectable,
        #[cfg(feature = "kvm")]
        DriverType::KVM => context.driver as *mut Kvm as *mut dyn Introspectable,
        #[cfg(feature = "virtualbox")]
        DriverType::VirtualBox => context.driver as *mut VBox as *mut dyn Introspectable,
        #[cfg(feature = "xen")]
        DriverType::Xen => context.driver as *mut Xen as *mut dyn Introspectable,
        #[cfg(feature = "hyper-v")]
        DriverType::HyperV => context.driver as *mut HyperV as *mut dyn Introspectable,
    }
}

unsafe fn get_driver_box(context: &MicrovmiContext) -> Box<dyn Introspectable> {
    match context.driver_type {
        DriverType::Dummy => Box::from_raw(context.driver as *mut Dummy) as Box<dyn Introspectable>,
        #[cfg(feature = "kvm")]
        DriverType::KVM => Box::from_raw(context.driver as *mut Kvm) as Box<dyn Introspectable>,
        #[cfg(feature = "virtualbox")]
        DriverType::VirtualBox => {
            Box::from_raw(context.driver as *mut VBox) as Box<dyn Introspectable>
        }
        #[cfg(feature = "xen")]
        DriverType::Xen => Box::from_raw(context.driver as *mut Xen) as Box<dyn Introspectable>,
        #[cfg(feature = "hyper-v")]
        DriverType::HyperV => {
            Box::from_raw(context.driver as *mut HyperV) as Box<dyn Introspectable>
        }
    }
}
