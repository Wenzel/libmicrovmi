use std::env;
use std::thread;
use std::time;
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

    // play with pause and resume
    println!("pausing the VM");
    drv.pause();
    println!("waiting 5 seconds...");
    let duration = time::Duration::from_millis(5000);
    thread::sleep(duration);
    println!("resuming the VM");
    drv.resume();

    // close driver
    drv.close();
}
