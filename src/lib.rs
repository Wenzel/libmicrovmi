//! libmicrovmi is a cross-platform unified virtual machine introsection interface, following a simple design to keep interoperability at heart.
//!
//! Click on this [book 📖](https://libmicrovmi.github.io/) to find our project documentation.

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
#[cfg(feature = "virtualbox")]
use driver::virtualbox::VBox;
#[cfg(feature = "xen")]
use driver::xen::Xen;
use errors::MicrovmiError;
#[cfg(feature = "kvm")]
use kvmi::create_kvmi;

pub fn init(
    driver_type: Option<DriverType>,
    init_params: Option<DriverInitParams>,
) -> Result<Box<dyn Introspectable>, MicrovmiError> {
    info!("Microvmi init");
    match driver_type {
        None => {
            // for each possible DriverType
            for drv_type in DriverType::into_enum_iter() {
                // try to init
                match init_driver(drv_type, init_params.clone()) {
                    Ok(driver) => {
                        return Ok(driver);
                    }
                    Err(e) => {
                        debug!("{:?} driver initialization failed: {}", drv_type, e);
                        continue;
                    }
                }
            }
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
    match driver_type {
        #[cfg(feature = "kvm")]
        DriverType::KVM => Ok(Box::new(Kvm::new(create_kvmi(), _init_params)?)),
        #[cfg(feature = "virtualbox")]
        DriverType::VirtualBox => Ok(Box::new(VBox::new(_init_params)?)),
        #[cfg(feature = "xen")]
        DriverType::Xen => Ok(Box::new(Xen::new(_init_params)?)),
        #[allow(unreachable_patterns)]
        _ => Err(MicrovmiError::DriverNotCompiled(driver_type)),
    }
}
