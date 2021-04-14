use crate::api::{DriverInitParam, DriverType, Introspectable, Registers};
use crate::microvmi::Microvmi;
use bitflags::_core::ptr::null_mut;
use cty::{c_char, size_t, uint16_t, uint64_t, uint8_t};
use std::convert::TryInto;
use std::ffi::{c_void, CStr, CString};
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
    // this function might be called multiple times
    // with try_init we avoid panicking
    env_logger::try_init().ok();
}

/// Entrypoint for libmicrovmi
/// Initializes a specific driver, or all drivers compiled and returns the first one that succeeded
///
/// In case of error, init_error will be allocated with the underlying error message.
///
/// # Safety
///
/// The init_error pointer should be freed with rs_cstring_free()
#[no_mangle]
pub unsafe extern "C" fn microvmi_init(
    domain_name: *const c_char,
    driver_type: *const DriverType,
    driver_init_option: *const DriverInitParamFFI,
    init_error: *mut *const c_char,
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

    match Microvmi::new(&safe_domain_name, optional_driver_type, init_option) {
        Ok(m) => Box::into_raw(Box::new(m)) as *mut c_void,
        Err(err) => {
            if !init_error.is_null() {
                (*init_error) = CString::new(format!("{}", err))
                    .expect("Failed to convert MicrovmiError to CString")
                    .into_raw();
            };
            null_mut()
        }
    }
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
    bytes_read: *mut uint64_t,
) -> bool {
    let driver = get_driver_mut_ptr(context);

    let mut bytes_read_local = 0;
    let res = (*driver)
        .read_physical(
            physical_address,
            slice::from_raw_parts_mut(buffer, size),
            &mut bytes_read_local,
        )
        .is_ok();
    // update bytes_read if not NULL
    if !bytes_read.is_null() {
        bytes_read.write(bytes_read_local);
    }
    res
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

/// return the concrete DriverType for the given Microvmi driver
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_get_driver_type(context: *mut c_void) -> DriverType {
    let drv = get_driver_mut_ptr(context);
    (*drv).get_driver_type()
}

unsafe fn get_driver_mut_ptr(context: *mut c_void) -> *mut dyn Introspectable {
    let driver: *mut *mut dyn Introspectable = context as *mut _;
    driver.read()
}

unsafe fn get_driver_box(context: *mut c_void) -> Box<Box<dyn Introspectable>> {
    Box::from_raw(context as *mut _)
}

/// Free a CString allocated by Rust (for ex. using `rust_string_to_c`)
///
/// # Safety
///
/// s must be allocated by rust, using `CString::new`
// Note: this function was taken from https://github.com/OISF/suricata/blob/62e665c8482c90b30f6edfa7b0f0eabf8a4fcc79/rust/src/common.rs#L69
#[no_mangle]
pub unsafe extern "C" fn rs_cstring_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    drop(CString::from_raw(s));
}
