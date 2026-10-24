use bitflags::_core::ptr::null_mut;
use cty::{c_char, size_t, uint16_t, uint32_t, uint64_t, uint8_t};

use std::ffi::{c_void, CString};
use std::slice;

use crate::api::events::{Event, EventReplyType, InterceptType};
use crate::api::params::DriverInitParams;
use crate::api::registers::Registers;
use crate::api::{Access, DriverType, Introspectable};
use crate::capi::params::DriverInitParamsFFI;
use crate::init;
use std::convert::TryFrom;

pub mod params;

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
    driver_type: *const DriverType,
    init_params: *const DriverInitParamsFFI,
    init_error: *mut *const c_char,
) -> *mut c_void {
    // check driver type
    let optional_driver_type: Option<DriverType> = if driver_type.is_null() {
        None
    } else {
        Some(driver_type.read())
    };
    // check init params
    let optional_init_params = if init_params.is_null() {
        Ok(None)
    } else {
        DriverInitParams::try_from(init_params.read()).map(Some)
    };

    match optional_init_params.and_then(|p| Ok(init(optional_driver_type, p)?)) {
        Ok(driver) => Box::into_raw(Box::new(driver)) as *mut c_void,
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
    if !context.is_null() {
        let _ = get_driver_box(context);
        // box destructor is called
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_pause(context: *mut c_void) -> bool {
    if context.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    (*driver).pause().is_ok()
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_resume(context: *mut c_void) -> bool {
    if context.is_null() {
        return false;
    }
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
    if context.is_null() {
        return false;
    }
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
pub unsafe extern "C" fn microvmi_write_physical(
    context: *mut c_void,
    physical_address: uint64_t,
    buffer: *mut uint8_t,
    size: size_t,
) -> bool {
    if context.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    let slice_buf = slice::from_raw_parts_mut(buffer, size);
    (*driver)
        .write_physical(physical_address, slice_buf)
        .is_ok()
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_get_max_physical_addr(
    context: *mut c_void,
    address_ptr: *mut uint64_t,
) -> bool {
    if context.is_null() {
        return false;
    }
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
pub unsafe extern "C" fn microvmi_get_vcpu_count(
    context: *mut c_void,
    count_ptr: *mut uint16_t,
) -> bool {
    if context.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    match (*driver).get_vcpu_count() {
        Ok(count) => {
            count_ptr.write(count);
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
    if context.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    match (*driver).read_registers(vcpu) {
        Ok(regs) => {
            registers.write(regs);
            true
        }
        Err(_) => false,
    }
}

/// Writes `*registers` to `vcpu`. Returns false on error, which is logged.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_write_registers(
    context: *mut c_void,
    vcpu: uint16_t,
    registers: *const Registers,
) -> bool {
    if context.is_null() || registers.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    match (*driver).write_registers(vcpu, registers.read()) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_write_registers: {}", e);
            false
        }
    }
}

/// Sets the allowed access of `paddr`'s page in `view`. Returns false on error, which is logged.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_set_page_access(
    context: *mut c_void,
    view: uint16_t,
    paddr: uint64_t,
    access: Access,
) -> bool {
    if context.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    match (*driver).set_page_access(view, paddr, access) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_set_page_access: {}", e);
            false
        }
    }
}

/// Writes the allowed access of `paddr`'s page in `view` to `*access`. Returns false on error,
/// which is logged.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_get_page_access(
    context: *mut c_void,
    view: uint16_t,
    paddr: uint64_t,
    access: *mut Access,
) -> bool {
    if context.is_null() || access.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    match (*driver).get_page_access(view, paddr) {
        Ok(a) => {
            access.write(a);
            true
        }
        Err(e) => {
            error!("microvmi_get_page_access: {}", e);
            false
        }
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_enable_memory_views(
    context: *mut c_void,
    enabled: bool,
) -> bool {
    if context.is_null() {
        return false;
    }
    match (*get_driver_mut_ptr(context)).enable_memory_views(enabled) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_enable_memory_views: {}", e);
            false
        }
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_create_memory_view(
    context: *mut c_void,
    view: *mut uint16_t,
) -> bool {
    if context.is_null() || view.is_null() {
        return false;
    }
    match (*get_driver_mut_ptr(context)).create_memory_view() {
        Ok(created) => {
            view.write(created);
            true
        }
        Err(e) => {
            error!("microvmi_create_memory_view: {}", e);
            false
        }
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_destroy_memory_view(
    context: *mut c_void,
    view: uint16_t,
) -> bool {
    if context.is_null() {
        return false;
    }
    match (*get_driver_mut_ptr(context)).destroy_memory_view(view) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_destroy_memory_view: {}", e);
            false
        }
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_switch_memory_view(
    context: *mut c_void,
    view: uint16_t,
) -> bool {
    if context.is_null() {
        return false;
    }
    match (*get_driver_mut_ptr(context)).switch_memory_view(view) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_switch_memory_view: {}", e);
            false
        }
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_remap_memory_page(
    context: *mut c_void,
    view: uint16_t,
    page: uint64_t,
    target: uint64_t,
) -> bool {
    if context.is_null() {
        return false;
    }
    match (*get_driver_mut_ptr(context)).remap_memory_page(view, page, target) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_remap_memory_page: {}", e);
            false
        }
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_allocate_hidden_page(
    context: *mut c_void,
    page: *mut uint64_t,
) -> bool {
    if context.is_null() || page.is_null() {
        return false;
    }
    match (*get_driver_mut_ptr(context)).allocate_hidden_page() {
        Ok(allocated) => {
            page.write(allocated);
            true
        }
        Err(e) => {
            error!("microvmi_allocate_hidden_page: {}", e);
            false
        }
    }
}

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_free_hidden_page(
    context: *mut c_void,
    page: uint64_t,
) -> bool {
    if context.is_null() {
        return false;
    }
    match (*get_driver_mut_ptr(context)).free_hidden_page(page) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_free_hidden_page: {}", e);
            false
        }
    }
}

/// Returns the Hidden page pool size (VBoxInternal/VMI/HiddenPages), or 0 if not applicable or on error.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_hidden_page_pool_size(context: *mut c_void) -> uint32_t {
    if context.is_null() {
        return 0;
    }
    (*get_driver_mut_ptr(context)).hidden_page_pool_size()
}

/// Enables or disables an intercept on a vCPU. Returns false on error, which is logged.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_toggle_intercept(
    context: *mut c_void,
    vcpu: uint16_t,
    intercept_type: InterceptType,
    enabled: bool,
) -> bool {
    if context.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    match (*driver).toggle_intercept(vcpu, intercept_type, enabled) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_toggle_intercept: {}", e);
            false
        }
    }
}

/// Queues an x86 #PF for a parked vCPU before replying to its event. Returns false on error;
/// the vCPU stays parked. `virtual_address` becomes CR2, `error_code` is the x86 PF error code.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_request_page_fault(
    context: *mut c_void,
    vcpu: uint16_t,
    virtual_address: uint64_t,
    error_code: uint32_t,
) -> bool {
    if context.is_null() {
        return false;
    }
    match (*get_driver_mut_ptr(context)).inject_page_fault(vcpu, virtual_address, error_code) {
        Ok(()) => true,
        Err(e) => {
            error!("microvmi_request_page_fault: {}", e);
            false
        }
    }
}

/// Waits up to `timeout_ms` for the next event. Returns false on error, which is logged. On
/// success, `*event` is the event, owned by the caller until it is passed to
/// `microvmi_reply_event`, or NULL after the timeout.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_listen(
    context: *mut c_void,
    timeout_ms: uint32_t,
    event: *mut *mut Event,
) -> bool {
    if context.is_null() || event.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    match (*driver).listen(timeout_ms) {
        Ok(ev) => {
            event.write(ev.map_or(null_mut(), |ev| Box::into_raw(Box::new(ev))));
            true
        }
        Err(e) => {
            error!("microvmi_listen: {}", e);
            false
        }
    }
}

/// Replies to an event from `microvmi_listen` and frees it on success. Returns false on error,
/// which is logged; the caller then still owns the event and may reply to it again (a failed
/// reply leaves the vCPU parked).
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_reply_event(
    context: *mut c_void,
    event: *mut Event,
    reply: EventReplyType,
) -> bool {
    if event.is_null() || context.is_null() {
        return false;
    }
    let driver = get_driver_mut_ptr(context);
    match (*driver).reply_event((*event).clone(), reply) {
        Ok(()) => {
            drop(Box::from_raw(event));
            true
        }
        Err(e) => {
            error!("microvmi_reply_event: {}", e);
            false
        }
    }
}

/// Checks if the event has registers and writes them to `*registers` if present.
/// Returns true if registers were available, false otherwise.
#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn microvmi_event_get_registers(
    event: *const Event,
    registers: *mut Registers,
) -> bool {
    if event.is_null() || registers.is_null() {
        return false;
    }
    match &(*event).regs {
        Some(regs) => {
            *registers = Registers::X86((**regs).clone());
            true
        }
        None => false,
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

#[allow(clippy::redundant_allocation)]
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
