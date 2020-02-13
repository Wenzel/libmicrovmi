pub mod api;
pub mod capi;
mod driver;

#[macro_use]
extern crate log;

#[cfg(feature = "hyper-v")]
use driver::hyperv::HyperVEvent;
#[cfg(feature = "kvm")]
use kvmi::KVMiEvent;
#[cfg(feature = "virtualbox")]
use driver::virtualbox::VBoxEvent;
#[cfg(feature = "xen")]
use driver::xen::XenEvent;

// define type alias for DriverEvent concrete type at compile time
#[cfg(feature = "hyper-v")]
pub type Ev = HyperVEvent;
#[cfg(feature = "kvm")]
pub type Ev = KVMiEvent;
#[cfg(feature = "virtualbox")]
pub type Ev = VBoxEvent;
#[cfg(feature = "xen")]
pub type Ev = XenEvent;

use api::DriverType;
use api::Introspectable;
#[cfg(feature = "hyper-v")]
use driver::hyperv::HyperV;
#[cfg(feature = "kvm")]
use driver::kvm::Kvm;
#[cfg(feature = "virtualbox")]
use driver::virtualbox::VBox;
#[cfg(feature = "xen")]
use driver::xen::Xen;

#[allow(unreachable_code)]
pub fn init(domain_name: &str, driver_type: Option<DriverType>) -> Box<dyn Introspectable<DriverEvent=Ev>> {
    debug!("Microvmi init");
    match driver_type {
        Some(drv_type) => match drv_type {
            #[cfg(feature = "hyper-v")]
            DriverType::HyperV => Box::new(HyperV::new(domain_name)) as Box<dyn Introspectable>,
            #[cfg(feature = "kvm")]
            DriverType::KVM => Box::new(Kvm::new(domain_name)) as Box<dyn Introspectable<DriverEvent=Ev>>,
            #[cfg(feature = "virtualbox")]
            DriverType::VirtualBox => Box::new(VBox::new(domain_name)) as Box<dyn Introspectable>,
            #[cfg(feature = "xen")]
            DriverType::Xen => Box::new(Xen::new(domain_name)) as Box<dyn Introspectable>,
        },
        None => {
            // test Hyper-V
            #[cfg(feature = "hyper-v")]
            {
                return Box::new(HyperV::new(domain_name)) as Box<dyn Introspectable>;
            }

            // test KVM
            #[cfg(feature = "kvm")]
            {
                return Box::new(Kvm::new(domain_name)) as Box<dyn Introspectable<DriverEvent=Ev>>;
            }

            // test VirtualBox
            #[cfg(feature = "virtualbox")]
            {
                return Box::new(VBox::new(domain_name)) as Box<dyn Introspectable>;
            }

            // test Xen
            #[cfg(feature = "xen")]
            {
                return Box::new(Xen::new(domain_name)) as Box<dyn Introspectable>;
            }
        }
    }
}
