use std::env;
extern crate microvmi;

// traits method can only be used if the trait is in the scope
use microvmi::api::Introspectable;
use microvmi::api::DriverType;

const PAGE_SIZE: usize = 4096;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <vm_name>", args[0]);
        return;
    }
    let domain_name = &args[1];

    let drv_type = DriverType::Xen;
    let drv: Box<Introspectable> = microvmi::init(drv_type, domain_name);

    println!("pausing the VM");
    drv.pause();

    let mut buffer: [u8; PAGE_SIZE] = [0; PAGE_SIZE];
    let mut cur_addr: u64 = 0;
    let max_addr = drv.get_max_physical_addr().unwrap();
    println!("Max address @{:x}", max_addr);
    while cur_addr < max_addr {
        let result = drv.read_physical(cur_addr, &mut buffer);
        match result {
            Ok(()) => println!("page read success 0x{:x}", cur_addr),
            Err(error) => println!("page read failed 0x{:x}", cur_addr),
        }
        cur_addr += PAGE_SIZE as u64;
    }

    println!("resuming the VM");
    drv.resume();
}
