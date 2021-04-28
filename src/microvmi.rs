//! This module defines the Microvmi struct which should be the entrypoint to interact with libmicrovmi

use enum_iterator::IntoEnumIterator;
#[cfg(feature = "kvm")]
use kvmi::create_kvmi;

use crate::api::{DriverInitParam, DriverType, Event, InterceptType};
use crate::api::{Introspectable, Registers};
#[cfg(feature = "kvm")]
use crate::driver::kvm::Kvm;
#[cfg(feature = "virtualbox")]
use crate::driver::virtualbox::VBox;
#[cfg(feature = "xen")]
use crate::driver::xen::Xen;
use crate::errors::MicrovmiError;
use crate::memory::Memory;
use crate::memory::PaddedMemory;
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

/// Main struct to interact with the library
pub struct Microvmi {
    // runtime VMI driver
    pub(crate) drv: Rc<RefCell<Box<dyn Introspectable>>>,
    /// Exposes the physical memory as a file-like interface
    pub memory: Memory,
    /// Exposes the physical memory as a file-like interface, with padding
    pub padded_memory: PaddedMemory,
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
    /// use crate::microvmi::microvmi::Microvmi;
    /// use crate::microvmi::api::{DriverType, DriverInitParam};
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
        let drv = match driver_type {
            None => {
                // for each possible DriverType
                let mut driver: Option<Box<dyn Introspectable>> = None;
                for drv_type in DriverType::into_enum_iter() {
                    // try to init
                    match init_driver(domain_name, drv_type, init_option.clone()) {
                        Ok(drv) => {
                            driver = Some(drv);
                            break;
                        }
                        Err(e) => {
                            debug!("{:?} driver initialization failed: {}", drv_type, e);
                            continue;
                        }
                    }
                }
                driver.ok_or(MicrovmiError::NoDriverAvailable)?
            }
            Some(drv_type) => init_driver(domain_name, drv_type, init_option)?,
        };
        let ref_drv = Rc::new(RefCell::new(drv));
        Ok(Microvmi {
            drv: ref_drv.clone(),
            memory: Memory::new(ref_drv.clone())?,
            padded_memory: PaddedMemory::new(ref_drv.clone())?,
        })
    }

    /// Get the maximum physical address
    ///
    /// Returns maximum physical address in 64 bit unsigned integer format.
    ///
    pub fn get_max_physical_addr(&self) -> Result<u64, Box<dyn Error>> {
        self.drv.borrow().get_max_physical_addr()
    }

    /// Read register values
    ///
    /// # Arguments
    /// * 'vcpu' - vcpu id for which the value of registers are to be dumped as the argument
    ///
    pub fn read_registers(&self, vcpu: u16) -> Result<Registers, Box<dyn Error>> {
        self.drv.borrow().read_registers(vcpu)
    }

    /// Pauses the VM
    pub fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        self.drv.borrow_mut().resume()
    }

    /// Resumes the VM
    pub fn resume(&mut self) -> Result<(), Box<dyn Error>> {
        self.drv.borrow_mut().resume()
    }

    /// Return the concrete DriverType
    pub fn get_driver_type(&self) -> DriverType {
        self.drv.borrow().get_driver_type()
    }

    /// Used to enable/disable an event interception
    ///
    /// # Arguments
    /// * 'vcpu' - vcpu id for which we are to enable/disable intercept monitoring
    /// * 'intercept_type' - to specify event type for which to raise flag
    /// * 'enabled' - flag to specify whether to enable/disable event monitoring
    ///
    pub fn toggle_intercept(
        &mut self,
        vcpu: u16,
        intercept_type: InterceptType,
        enabled: bool,
    ) -> Result<(), Box<dyn Error>> {
        self.drv
            .borrow_mut()
            .toggle_intercept(vcpu, intercept_type, enabled)
    }

    /// Listen and return the next event, or None
    ///
    /// # Arguments
    /// * 'timeout' - Time for which it will wait for a new event
    ///
    pub fn listen(&mut self, timeout: u32) -> Result<Option<Event>, Box<dyn Error>> {
        self.drv.borrow_mut().listen(timeout)
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
