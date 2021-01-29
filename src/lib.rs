//! Today the VMI ecosystem is made of a multitude of applications, targeting one hypervisor or emulator, with their own semantic library. (Examples includes Drakvuf, PANDA, PyREBox, icebox, etc...). If we want to make the most out of VMI in the future, we need to build the libraries that will unify this ecosystem and let the developers focus on what matters: building quality VMI apps. This is where libmicrovmi comes into play. It aims to solve this problem, by providing a core, foundation library, written in Rust, to be cross-platform, hypervisor-agnostic and emulator-agnostic, on top of which higher-level libraries and apps can rebase. Rust makes a lot of sense for VMI for 2 main reasons: Rust is safe: considering that we are processing untrusted input from virtual machines, we cannot allow any crash or exploitation in the introspection agent. Also one of our use case is OS hardening, which needs an excellent level of trust Rust is fast: processing an event requires to pause the VCPU. The longer the pause, the more delayed the guest execution will be, and when scaling to thousands of events per second this can dramatically influence how many breakpoints you are willing to put, especially on production systems. Speed matters. Therefore Rust is the de facto choice for VMI apps in the future, and we are building it today, by providing libmicrovmi, a new foundation for VMI. Libmicrovmi has drivers for: Xen KVM Hyper-V (in progress)
pub mod api;
pub mod capi;
mod driver;
pub mod errors;

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
