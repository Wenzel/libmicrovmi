use crate::api::{DriverType, Introspectable, Registers};
use crate::init;
use cty::{c_char, size_t, uint16_t, uint64_t, uint8_t};
use std::ffi::{c_void, CStr};
use std::slice;

#[repr(C)]
pub enum MicrovmiStatus {
    MicrovmiSuccess,
    MicrovmiFailure,
}

/// This API allows a C program to initialize the logging system in libmicrovmi.
/// This simply calls env_logger::init()
/// Usually, it's the library consumer who should add this Rust crate dependency,
/// however, with a C program, we provide this workaround where we provide an API to do just that.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_envlogger_init() {
    env_logger::init();
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_init(
    domain_name: *const c_char,
    driver_type: *const DriverType,
) -> *mut c_void {
    let safe_domain_name = CStr::from_ptr(domain_name).to_string_lossy().into_owned();
    let optional_driver_type: Option<DriverType> = if driver_type.is_null() {
        None
    } else {
        Some(driver_type.read())
    };
    let driver = init(&safe_domain_name, optional_driver_type);
    Box::into_raw(Box::new(driver)) as *mut c_void
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_destroy(context: *mut c_void) {
    let _ = get_driver_box(context);
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_pause(context: *mut c_void) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context);
    match (*driver).pause() {
        Ok(_) => MicrovmiStatus::MicrovmiSuccess,
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_resume(context: *mut c_void) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context);
    match (*driver).resume() {
        Ok(_) => MicrovmiStatus::MicrovmiSuccess,
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_read_physical(
    context: *mut c_void,
    physical_address: uint64_t,
    buffer: *mut uint8_t,
    size: size_t,
) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context);
    match (*driver).read_physical(physical_address, slice::from_raw_parts_mut(buffer, size)) {
        Ok(_) => MicrovmiStatus::MicrovmiSuccess,
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_get_max_physical_addr(
    context: *mut c_void,
    address_ptr: *mut uint64_t,
) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context);
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
    context: *mut c_void,
    vcpu: uint16_t,
    registers: *mut Registers,
) -> MicrovmiStatus {
    let driver = get_driver_mut_ptr(context);
    match (*driver).read_registers(vcpu) {
        Ok(regs) => {
            registers.write(regs);
            MicrovmiStatus::MicrovmiSuccess
        }
        Err(_) => MicrovmiStatus::MicrovmiFailure,
    }
}

unsafe fn get_driver_mut_ptr(context: *mut c_void) -> *mut dyn Introspectable {
    let driver: *mut *mut dyn Introspectable = context as *mut _;
    driver.read()
}

unsafe fn get_driver_box(context: *mut c_void) -> Box<Box<dyn Introspectable>> {
    Box::from_raw(context as *mut _)
}
