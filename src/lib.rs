pub mod api;
mod driver;

use api::Introspectable;
use api::DriverType;
use driver::dummy::Dummy;
#[cfg(feature="xen")]
use driver::xen::Xen;
#[cfg(feature="kvm")]
use driver::kvm::Kvm;

pub fn init(driver_type: DriverType, domain_name: &String) -> Box<Introspectable> {
    println!("vmi init");

    match driver_type {
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
    }
}
