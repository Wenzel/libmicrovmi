extern crate microvmi;

// traits method can only be used if the trait is in the scope
use microvmi::api::Introspectable;

fn main() {
    println!("hello world !");

    let drv = microvmi::vmi_init();
    // close driver
    drv.close();
}
