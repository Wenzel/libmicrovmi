//! libmicrovmi is a cross-platform unified virtual machine introsection interface, following a simple design to keep interoperability at heart.
//!
//! Click on this [book 📖](https://libmicrovmi.github.io/) to find our project documentation.

pub mod api;
pub mod capi;
mod driver;
pub mod errors;
pub mod pyapi;

#[macro_use]
extern crate log;
#[macro_use]
extern crate bitflags;

use enum_iterator::IntoEnumIterator;

use api::Introspectable;
use api::{DriverInitParam, DriverType};
#[cfg(feature = "hyper-v")]
use driver::hyperv::HyperV;
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
    domain_name: &str,
    driver_type: Option<DriverType>,
    init_option: Option<DriverInitParam>,
) -> Result<Box<dyn Introspectable>, MicrovmiError> {
    info!("Microvmi init");
    match driver_type {
        None => {
            // for each possible DriverType
            for drv_type in DriverType::into_enum_iter() {
                // try to init
                match init_driver(domain_name, drv_type, init_option.clone()) {
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
        Some(drv_type) => init_driver(domain_name, drv_type, init_option),
    }
}

/// Initialize a given driver type
/// return None if the requested driver has not been compiled in libmicrovmi
fn init_driver(
    _domain_name: &str,
    driver_type: DriverType,
    _init_option: Option<DriverInitParam>,
) -> Result<Box<dyn Introspectable>, MicrovmiError> {
    #[allow(clippy::match_single_binding)]
    match driver_type {
        #[cfg(feature = "hyper-v")]
        DriverType::HyperV => Ok(Box::new(HyperV::new(_domain_name, _init_option)?)),
        #[cfg(feature = "kvm")]
        DriverType::KVM => Ok(Box::new(Kvm::new(
            _domain_name,
            create_kvmi(),
            _init_option,
        )?)),
        #[cfg(feature = "virtualbox")]
        DriverType::VirtualBox => Ok(Box::new(VBox::new(_domain_name, _init_option)?)),
        #[cfg(feature = "xen")]
        DriverType::Xen => Ok(Box::new(Xen::new(_domain_name, _init_option)?)),
        _ => Err(MicrovmiError::DriverNotCompiled(driver_type)),
    }
}
