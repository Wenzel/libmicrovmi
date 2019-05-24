extern crate microvmi;

// traits method can only be used if the trait is in the scope
use microvmi::api::Introspectable;
use microvmi::api::DriverType;

fn main() {
    println!("hello world !");


    // instantiate the dummy driver
    let drv_type = DriverType::Dummy;
    let drv = microvmi::init(drv_type);
    // close driver
    drv.close();
}
