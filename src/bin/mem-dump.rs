use std::env;
use std::path::Path;
use std::fs::File;
use std::io::Write;
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
    let dump_name = format!("{}.dump", domain_name);
    let dump_path = Path::new(&dump_name);
    let mut dump_file = File::create(dump_path).expect("Fail to open dump file");

    let drv_type = DriverType::Dummy;
    let mut drv: Box<dyn Introspectable> = microvmi::init(drv_type, domain_name);

    println!("pausing the VM");
    drv.pause().expect("Failed to pause VM");

    let mut buffer: [u8; PAGE_SIZE] = [0; PAGE_SIZE];
    let max_addr = drv.get_max_physical_addr().unwrap();
    println!("Max address @{:x}", max_addr);
    println!("Dumping physical memory to {}", dump_path.display());
    for cur_addr in (0..max_addr).step_by(PAGE_SIZE) {
        let result = drv.read_physical(cur_addr, &mut buffer);
        match result {
            Ok(()) => {
                dump_file.write_all(&buffer).expect("failed to write to file");
            },
            Err(_error) => (),
        }
    }

    println!("resuming the VM");
    drv.resume().expect("Failed to resume VM");
}
