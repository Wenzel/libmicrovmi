//! Today the VMI ecosystem is made of a multitude of applications, targeting one hypervisor or emulator, with their own semantic library. (Examples includes Drakvuf, PANDA, PyREBox, icebox, etc...). If we want to make the most out of VMI in the future, we need to build the libraries that will unify this ecosystem and let the developers focus on what matters: building quality VMI apps. This is where libmicrovmi comes into play. It aims to solve this problem, by providing a core, foundation library, written in Rust, to be cross-platform, hypervisor-agnostic and emulator-agnostic, on top of which higher-level libraries and apps can rebase. Rust makes a lot of sense for VMI for 2 main reasons: Rust is safe: considering that we are processing untrusted input from virtual machines, we cannot allow any crash or exploitation in the introspection agent. Also one of our use case is OS hardening, which needs an excellent level of trust Rust is fast: processing an event requires to pause the VCPU. The longer the pause, the more delayed the guest execution will be, and when scaling to thousands of events per second this can dramatically influence how many breakpoints you are willing to put, especially on production systems. Speed matters. Therefore Rust is the de facto choice for VMI apps in the future, and we are building it today, by providing libmicrovmi, a new foundation for VMI. Libmicrovmi has drivers for: Xen KVM Hyper-V (in progress)
pub mod api;
pub mod capi;
mod driver;

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
#[cfg(feature = "kvm")]
use kvmi::create_kvmi;

pub fn init(
    domain_name: &str,
    driver_type: Option<DriverType>,
    init_option: Option<DriverInitParam>,
) -> Box<dyn Introspectable> {
    info!("Microvmi init");
    match driver_type {
        None => {
            // for each possible DriverType
            for drv_type in DriverType::into_enum_iter() {
                // try to init
                // TODO: the driver init should return a Result
                match init_driver(domain_name, drv_type, init_option.clone()) {
                    Some(driver) => {
                        return driver;
                    }
                    None => {
                        info!(
                            "Driver {:?} not compiled in libmicrovmi. Skipping.",
                            drv_type
                        );
                        continue;
                    }
                }
            }
            // TODO: to Err
            panic!("No suitable libmicrovmi driver avaialble !");
        }
        Some(drv_type) => match init_driver(domain_name, drv_type, init_option) {
            None => panic!(
                "Selected driver {:?} has not been compiled in libmicrovmi",
                drv_type
            ),
            Some(driver) => driver,
        },
    }
}

#[cfg(feature = "kvm")]
fn create_kvm(domain_name: &str, init_option: Option<DriverInitParam>) -> Box<dyn Introspectable> {
    Box::new(Kvm::new(domain_name, create_kvmi(), init_option).unwrap()) as Box<dyn Introspectable>
}

/// Initialize a given driver type
/// return None if the requested driver has not been compiled in libmicrovmi
fn init_driver(
    domain_name: &str,
    driver_type: DriverType,
    init_option: Option<DriverInitParam>,
) -> Option<Box<dyn Introspectable>> {
    match driver_type {
        #[cfg(feature = "hyper-v")]
        DriverType::HyperV => {
            Some(Box::new(HyperV::new(domain_name, init_option)) as Box<dyn Introspectable>)
        }
        #[cfg(feature = "kvm")]
        DriverType::KVM => Some(create_kvm(domain_name, init_option)),
        #[cfg(feature = "virtualbox")]
        DriverType::VirtualBox => {
            Some(Box::new(VBox::new(domain_name, init_option)) as Box<dyn Introspectable>)
        }
        #[cfg(feature = "xen")]
        DriverType::Xen => {
            Some(Box::new(Xen::new(domain_name, init_option)) as Box<dyn Introspectable>)
        }
        _ => None,
    }
}
