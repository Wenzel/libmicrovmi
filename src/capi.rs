use crate::api::{
    Access, DriverInitParam, DriverType, Event, EventReplyType, InterceptType, Introspectable,
    Registers,
};
use crate::init;
use cty::{c_char, size_t, uint16_t, uint32_t, uint64_t, uint8_t};
use std::convert::TryInto;
use std::ffi::{c_void, CStr};
use std::slice;

/// Support passing initialization options
/// similar to DriverInitParam, however this enum offers C API compatibility
#[repr(C)]
#[derive(Debug)]
pub enum DriverInitParamFFI {
    KVMiSocket(*const c_char),
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
    driver_init_option: *const DriverInitParamFFI,
) -> *mut c_void {
    let safe_domain_name = CStr::from_ptr(domain_name).to_string_lossy().into_owned();
    let optional_driver_type: Option<DriverType> = if driver_type.is_null() {
        None
    } else {
        Some(driver_type.read())
    };
    let init_option: Option<DriverInitParam> = if driver_init_option.is_null() {
        None
    } else {
        Some(
            DriverInitParamFFI::try_into(driver_init_option.read())
                .expect("Failed to convert DriverInitParam C struct to Rust equivalent"),
        )
    };
    let driver = init(&safe_domain_name, optional_driver_type, init_option);
    Box::into_raw(Box::new(driver)) as *mut c_void
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_destroy(context: *mut c_void) {
    let _ = get_driver_box(context);
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_pause(context: *mut c_void) -> bool {
    let driver = get_driver_mut_ptr(context);
    (*driver).pause().is_ok()
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_resume(context: *mut c_void) -> bool {
    let driver = get_driver_mut_ptr(context);
    (*driver).resume().is_ok()
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_read_physical(
    context: *mut c_void,
    physical_address: uint64_t,
    buffer: *mut uint8_t,
    size: size_t,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    (*driver)
        .read_physical(physical_address, slice::from_raw_parts_mut(buffer, size))
        .is_ok()
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_write_physical(
    context: *mut c_void,
    physical_address: uint64_t,
    buffer: *mut uint8_t,
    size: size_t,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    (*driver)
        .write_physical(physical_address, slice::from_raw_parts_mut(buffer, size))
        .is_ok()
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_get_max_physical_addr(
    context: *mut c_void,
    address_ptr: *mut uint64_t,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    match (*driver).get_max_physical_addr() {
        Ok(max_addr) => {
            address_ptr.write(max_addr);
            true
        }
        Err(_) => false,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_read_registers(
    context: *mut c_void,
    vcpu: uint16_t,
    registers: *mut Registers,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    match (*driver).read_registers(vcpu) {
        Ok(regs) => {
            registers.write(regs);
            true
        }
        Err(_) => false,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_write_registers(
    context: *mut c_void,
    vcpu: uint16_t,
    registers: Registers,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    (*driver).write_registers(vcpu, registers).is_ok()
}

/*#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_get_page_access(
    context: *mut c_void,
    paddr: uint64_t,
    access: *mut Access,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    match (*driver).get_page_access(paddr) {
        Ok(flags) => {
            access.write(flags);
            true
        }
        Err(_) => false,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_set_page_access(
    context: *mut c_void,
    paddr: uint64_t,
    access: Access,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    (*driver).set_page_access(paddr, access).is_ok()
}*/

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_toggle_intercept(
    context: *mut c_void,
    vcpu: uint16_t,
    intercept_type: InterceptType,
    enabled: bool,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    (*driver)
        .toggle_intercept(vcpu, intercept_type, enabled)
        .is_ok()
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_listen(
    context: *mut c_void,
    timeout: uint32_t,
    event_ptr: *mut Event,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    match (*driver).listen(timeout) {
        Ok(Some(event)) => {
            event_ptr.write(event);
            true
        }
        Ok(None) => true,
        Err(_) => false,
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_reply_event(
    context: *mut c_void,
    event: Event,
    reply_type: EventReplyType,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    (*driver).reply_event(event, reply_type).is_ok()
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_get_vcpu_count(
    context: *mut c_void,
    vcpu_ptr: *mut uint16_t,
) -> bool {
    let driver = get_driver_mut_ptr(context);
    match (*driver).get_vcpu_count() {
        Ok(vcpu_count) => {
            vcpu_ptr.write(vcpu_count);
            true
        }
        Err(_) => false,
    }
}

unsafe fn get_driver_mut_ptr(context: *mut c_void) -> *mut dyn Introspectable {
    let driver: *mut *mut dyn Introspectable = context as *mut _;
    driver.read()
}

unsafe fn get_driver_box(context: *mut c_void) -> Box<Box<dyn Introspectable>> {
    Box::from_raw(context as *mut _)
}
