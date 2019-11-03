use std::env;
extern crate microvmi;

// traits method can only be used if the trait is in the scope
use microvmi::api::{Introspectable, Registers};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <vm_name>", args[0]);
        return;
    }
    let domain_name = &args[1];

    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);

    println!("pausing the VM");
    drv.pause().expect("Failed to pause VM");

    println!("dumping registers on VCPU 0");
    let regs: Registers = drv.read_registers(0)
        .expect("Failed to read registers");
    println!("{:#x?}", regs);

    println!("resuming the VM");
    drv.resume().expect("Failed to resume VM");
}
