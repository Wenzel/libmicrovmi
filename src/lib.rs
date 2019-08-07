pub mod api;
mod driver;

use api::Introspectable;
use api::DriverType;
use driver::dummy::Dummy;
#[cfg(feature="xen")]
use driver::xen::Xen;
#[cfg(feature="kvm")]
use driver::kvm::Kvm;

pub fn init(domain_name: &String, driver_type: Option<DriverType>) -> Box<Introspectable> {
    println!("vmi init");

    match driver_type {
        Some(drv_type) => match drv_type {
            DriverType::Dummy => {
                Box::new(Dummy::new(domain_name)) as Box<Introspectable>
            },
            #[cfg(feature="xen")]
            DriverType::Xen => {
                Box::new(Xen::new(domain_name)) as Box<Introspectable>
            },
            #[cfg(feature="kvm")]
            DriverType::KVM => {
                Box::new(Kvm::new(domain_name)) as Box<Introspectable>
            },
        },
        None => {
            // test Xen
            #[cfg(feature="xen")] {
                return Box::new(Xen::new(domain_name)) as Box<Introspectable>;
            }

            // test KVM
            #[cfg(feature="kvm")] {
                return Box::new(Kvm::new(domain_name)) as Box<Introspectable>;
            }

            // return Dummy if no other driver has been compiled
            return Box::new(Dummy::new(domain_name)) as Box<Introspectable>;
        }
    }
}
