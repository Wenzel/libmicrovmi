//! libmicrovmi is a cross-platform unified virtual machine introsection interface, following a simple design to keep interoperability at heart.
//!
//! Click on this [book 📖](https://wenzel.github.io/libmicrovmi/) to find our project documentation.
//!
//! The library's entrypoint is the [init](fn.init.html) function.

#![allow(clippy::upper_case_acronyms)]

pub mod api;
pub mod capi;
mod driver;
pub mod errors;

#[macro_use]
extern crate log;
#[macro_use]
extern crate bitflags;

use enum_iterator::IntoEnumIterator;

use api::params::DriverInitParams;
use api::DriverType;
use api::Introspectable;
#[cfg(feature = "kvm")]
use driver::kvm::Kvm;
#[cfg(feature = "mflow")]
use driver::memflow::Memflow;
#[cfg(feature = "virtualbox")]
use driver::virtualbox::VBox;
#[cfg(feature = "xen")]
use driver::xen::Xen;
use errors::MicrovmiError;
#[cfg(feature = "kvm")]
use kvmi::create_kvmi;

/// libmicrovmi initialization entrypoint
///
/// This function will initialize a libmicrovmi driver and call the hypervisor VMI API.
/// It returns a `Box<dyn Introspectable>` trait object, which implements the [Introspectable](api/trait.Introspectable.html) trait.
///
/// For complete documentation on driver init params, please check [DriverInitParams](struct.DriverInitParams.html) struct.
///
/// # Arguments
/// * `driver_type`: optional driver type to initialize. If None, all compiled drivers will be initialized one by one. The first that succeeds will be returned.
/// * `init_params`: optional driver initialization parameters
///
/// # Examples
/// ```no_run
/// use microvmi::init;
/// // 1 - attempt to init all drivers, without any init parameters
/// let drv = init(None, None);
///
/// // 2 - add parameters: vm_name
/// // a `vm_name` parameter is required for multiple drivers: Xen, KVM, VirtualBox
/// use microvmi::api::params::{DriverInitParams, CommonInitParams};
/// let init_params = DriverInitParams {
///     common: Some(CommonInitParams { vm_name: String::from("windows10")}),
///     ..Default::default()
/// };
/// let drv = init(None, Some(init_params));
///
/// // 3 - add parameters: KVM specific params
/// // KVM requires an additional unix socket to be specified
/// // and specify the KVM driver only
/// use microvmi::api::params::KVMInitParams;
/// use microvmi::api::DriverType;
/// let init_params = DriverInitParams {
///     common: Some(CommonInitParams { vm_name: String::from("windows10")}),
///     kvm: Some(KVMInitParams::UnixSocket {path: String::from("/tmp/introspector")}),
///     ..Default::default()
/// };
/// let drv = init(Some(DriverType::KVM), Some(init_params));
/// ```
pub fn init(
    driver_type: Option<DriverType>,
    init_params: Option<DriverInitParams>,
) -> Result<Box<dyn Introspectable>, MicrovmiError> {
    info!("Microvmi init");
    debug!("Microvmi init params: {:#?}", init_params);
    match driver_type {
        None => {
            // for each possible DriverType
            for drv_type in DriverType::into_enum_iter() {
                // try to init
                match init_driver(drv_type, init_params.clone()) {
                    Ok(driver) => {
                        return Ok(driver);
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }
            info!("No driver available");
            Err(MicrovmiError::NoDriverAvailable)
        }
        Some(drv_type) => init_driver(drv_type, init_params),
    }
}

/// Initialize a given driver type
/// return None if the requested driver has not been compiled in libmicrovmi
fn init_driver(
    driver_type: DriverType,
    init_params_option: Option<DriverInitParams>,
) -> Result<Box<dyn Introspectable>, MicrovmiError> {
    let _init_params = init_params_option.unwrap_or(DriverInitParams {
        ..Default::default()
    });
    #[allow(clippy::match_single_binding)]
    let res: Result<Box<dyn Introspectable>, MicrovmiError> = match driver_type {
        #[cfg(feature = "kvm")]
        DriverType::KVM => Ok(Box::new(Kvm::new(create_kvmi(), _init_params)?)),
        #[cfg(feature = "mflow")]
        DriverType::Memflow => Ok(Box::new(Memflow::new(_init_params)?)),
        #[cfg(feature = "virtualbox")]
        DriverType::VirtualBox => Ok(Box::new(VBox::new(_init_params)?)),
        #[cfg(feature = "xen")]
        DriverType::Xen => Ok(Box::new(Xen::new(_init_params)?)),
        #[allow(unreachable_patterns)]
        _ => Err(MicrovmiError::DriverNotCompiled(driver_type)),
    };
    match res {
        Ok(ref driver) => {
            info!("Driver initialized: {:?}", driver.get_driver_type());
        }
        Err(ref e) => {
            debug!("{:?} driver initialization failed: {}", driver_type, e);
        }
    }
    res
}
