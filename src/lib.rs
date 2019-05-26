pub mod api;
mod driver;

use api::Introspectable;
use api::DriverType;

pub fn init(driver_type: DriverType) -> impl Introspectable {
    println!("vmi init");

    // match arms have to return the same type
    // it doesnt like Traits, so be have to hide it in a Box
    let drv;
    match driver_type {
        DriverType::Dummy => {
            drv = Box::new(driver::dummy::Dummy) as Box<Introspectable>
        },
        DriverType::Xen => {
            drv = Box::new(driver::xen::Xen) as Box<Introspectable>
        },
    }
    // unbox it
    *drv;
}
