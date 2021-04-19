//! This module defines the Microvmi struct which should be the entrypoint to interact with libmicrovmi

use enum_iterator::IntoEnumIterator;
#[cfg(feature = "kvm")]
use kvmi::create_kvmi;

use crate::api::Introspectable;
use crate::api::{DriverInitParam, DriverType};
use crate::errors::MicrovmiError;
#[cfg(feature = "kvm")]
use driver::kvm::Kvm;
#[cfg(feature = "virtualbox")]
use driver::virtualbox::VBox;
#[cfg(feature = "xen")]
use driver::xen::Xen;
use std::error::Error;

/// Main struct to interact with the library
pub struct Microvmi {
    // runtime VMI driver
    pub(crate) drv: Box<dyn Introspectable>,
    // position in the physical memory (seek)
    pub(crate) pos: u64,
}

impl Microvmi {
    /// Initializes a new Microvmi instance
    ///
    /// # Arguments
    ///
    /// * `domain_name` - The domain name
    /// * `driver_type` - The driver type to initialize. None will attempt to initialize every driver avaiable
    /// * `init_option` - Initialization parameters for the driver.
    ///
    /// # Example
    ///
    /// ```
    /// use self::microvmi::Microvmi;
    /// use crate::api::{DriverType, DriverInitParam};
    /// Microvmi::new("win10", None, None);
    /// Microvmi::new("win10", Some(DriverType::Xen), None);
    /// Microvmi::new("win10", Some(DriverType::KVM), Some(DriverInitParam::KVMiSocket("/tmp/introspector".to_string())));
    /// ```
    pub fn new(
        domain_name: &str,
        driver_type: Option<DriverType>,
        init_option: Option<DriverInitParam>,
    ) -> Result<Microvmi, MicrovmiError> {
        info!("Microvmi init");
        match driver_type {
            None => {
                // for each possible DriverType
                for drv_type in DriverType::into_enum_iter() {
                    // try to init
                    match init_driver(domain_name, drv_type, init_option.clone()) {
                        Ok(drv) => return { Ok(Microvmi { drv, pos: 0 }) },
                        Err(e) => {
                            debug!("{:?} driver initialization failed: {}", drv_type, e);
                            continue;
                        }
                    }
                }
                Err(MicrovmiError::NoDriverAvailable)
            }
            Some(drv_type) => {
                let drv = init_driver(domain_name, drv_type, init_option)?;
                return { Ok(Microvmi { drv, pos: 0 }) };
            }
        }
    }

    pub fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        Ok(self.drv.get_max_physical_addr()?)
    }

    pub fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(self.drv.pause()?)
    }

    pub fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(self.drv.resume()?)
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
        #[allow(unreachable_patterns)]
        _ => Err(MicrovmiError::DriverNotCompiled(driver_type)),
    }
}
