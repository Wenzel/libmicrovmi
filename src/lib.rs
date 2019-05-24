pub mod api;
mod driver;

use api::Introspectable;
use api::DriverType;

pub fn init(driver_type: DriverType) -> impl Introspectable {
    println!("vmi init");

    let drv;
    match driver_type {
        DriverType::Dummy => {
            drv = driver::dummy::Dummy;
        }
    }
    // instantiate the driver
    drv.new();
    drv
}
