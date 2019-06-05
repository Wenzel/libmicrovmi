use std::env;
extern crate microvmi;

// traits method can only be used if the trait is in the scope
use microvmi::api::Introspectable;
use microvmi::api::DriverType;

fn main() {
    let args: Vec<String> = env::args().collect();
    let domid = args[1].parse::<u32>().unwrap();
    println!("hello world !");


    let drv_type = DriverType::Xen;
    let mut drv: Box<Introspectable> = microvmi::init(drv_type, domid);
    // close driver
    drv.close();
}
