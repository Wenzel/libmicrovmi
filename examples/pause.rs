use std::env;
use std::{thread, time};

extern crate microvmi;
use env_logger;

// traits method can only be used if the trait is in the scope
use microvmi::api::Introspectable;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        println!("Usage: {} <vm_name> <timeout>", args[0]);
        return;
    }
    let domain_name = &args[1];
    let timeout = args[2].parse::<u64>().unwrap();

    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);

    println!("pausing VM for {} seconds", timeout);
    drv.pause().expect("Failed to pause VM");

    let duration = time::Duration::new(timeout, 0);
    thread::sleep(duration);

    println!("resuming VM");
    drv.resume().expect("Failed to resume VM");
}
