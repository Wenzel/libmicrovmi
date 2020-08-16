pub mod api;
pub mod capi;
mod driver;

#[macro_use]
extern crate log;

use api::Introspectable;
use api::{DriverInitParam, DriverType};
use driver::dummy::Dummy;
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

#[allow(unreachable_code)]
pub fn init(
    domain_name: &str,
    driver_type: Option<DriverType>,
    init_option: Option<DriverInitParam>,
) -> Box<dyn Introspectable> {
    debug!("Microvmi init");
    match driver_type {
        Some(drv_type) => match drv_type {
            DriverType::Dummy => {
                Box::new(Dummy::new(domain_name, init_option)) as Box<dyn Introspectable>
            }
            #[cfg(feature = "hyper-v")]
            DriverType::HyperV => {
                Box::new(HyperV::new(domain_name, init_option)) as Box<dyn Introspectable>
            }
            #[cfg(feature = "kvm")]
            DriverType::KVM => create_kvm(domain_name, init_option),
            #[cfg(feature = "virtualbox")]
            DriverType::VirtualBox => {
                Box::new(VBox::new(domain_name, init_option)) as Box<dyn Introspectable>
            }
            #[cfg(feature = "xen")]
            DriverType::Xen => {
                Box::new(Xen::new(domain_name, init_option)) as Box<dyn Introspectable>
            }
        },
        None => {
            // test Hyper-V
            #[cfg(feature = "hyper-v")]
            {
                return Box::new(HyperV::new(domain_name, init_option)) as Box<dyn Introspectable>;
            }

            // test KVM
            #[cfg(feature = "kvm")]
            {
                return create_kvm(domain_name, init_option);
            }

            // test VirtualBox
            #[cfg(feature = "virtualbox")]
            {
                return Box::new(VBox::new(domain_name, init_option)) as Box<dyn Introspectable>;
            }

            // test Xen
            #[cfg(feature = "xen")]
            {
                return Box::new(Xen::new(domain_name, init_option)) as Box<dyn Introspectable>;
            }
            // return Dummy if no other driver has been compiled
            Box::new(Dummy::new(domain_name, init_option)) as Box<dyn Introspectable>
        }
    }
}

#[cfg(feature = "kvm")]
fn create_kvm(domain_name: &str, init_option: Option<DriverInitParam>) -> Box<dyn Introspectable> {
    Box::new(Kvm::new(domain_name, create_kvmi(), init_option).unwrap()) as Box<dyn Introspectable>
}
