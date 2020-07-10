use std::env;
extern crate env_logger;
extern crate microvmi;

// traits method can only be used if the trait is in the scope
use microvmi::api::Introspectable;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <vm_name>", args[0]);
        return;
    }
    let domain_name = &args[1];

    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);

    println!("pausing the VM");
    drv.pause().expect("Failed to pause VM");
    let total_vcpu_count: u16 = drv.get_vcpu_count().expect("Failed to get vcpu count");
    for vcpu in 0..total_vcpu_count {
        println!("dumping registers on VCPU {}", vcpu);
        let regs = drv.read_registers(vcpu).expect("Failed to read registers");
        println!("{:#x?}", regs);
    }

    println!("resuming the VM");
    drv.resume().expect("Failed to resume VM");
}
