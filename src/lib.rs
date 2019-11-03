pub mod api;
mod driver;

#[macro_use]
extern crate log;

use api::Introspectable;
use api::DriverType;
use driver::dummy::Dummy;
#[cfg(feature="xen")]
use driver::xen::Xen;
#[cfg(feature="kvm")]
use driver::kvm::Kvm;

#[allow(unreachable_code)]
pub fn init(domain_name: &str, driver_type: Option<DriverType>) -> Box<dyn Introspectable> {
    debug!("Microvmi init");
    match driver_type {
        Some(drv_type) => match drv_type {
            DriverType::Dummy => {
                Box::new(Dummy::new(domain_name)) as Box<dyn Introspectable>
            },
            #[cfg(feature="xen")]
            DriverType::Xen => {
                Box::new(Xen::new(domain_name)) as Box<dyn Introspectable>
            },
            #[cfg(feature="kvm")]
            DriverType::KVM => {
                Box::new(Kvm::new(domain_name)) as Box<dyn Introspectable>
            },
        },
        None => {
            // test Xen
            #[cfg(feature="xen")] {
                return Box::new(Xen::new(domain_name)) as Box<dyn Introspectable>;
            }

            // test KVM
            #[cfg(feature="kvm")] {
                return Box::new(Kvm::new(domain_name)) as Box<dyn Introspectable>;
            }

            // return Dummy if no other driver has been compiled
            Box::new(Dummy::new(domain_name)) as Box<dyn Introspectable>
        }
    }
}
