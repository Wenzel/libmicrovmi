use std::fs::File;
use std::io::Write;
use std::path::Path;

use clap::{App, Arg, ArgMatches};
use env_logger;

use microvmi::api::{DriverInitParam, Introspectable};

const PAGE_SIZE: usize = 4096;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .author("Mathieu Tarral")
        .about("Dumps VM physical memory")
        .arg(Arg::with_name("vm_name").index(1).required(true))
        .arg(
            Arg::with_name("kvmi_socket")
                .short("k")
                .takes_value(true)
                .help(
                "pass additional KVMi socket initialization parameter required for the KVM driver",
            ),
        )
        .get_matches()
}

fn main() {
    env_logger::init();

    let matches = parse_args();
    let domain_name = matches.value_of("vm_name").unwrap();

    let dump_name = format!("{}.dump", domain_name);
    let dump_path = Path::new(&dump_name);
    let mut dump_file = File::create(dump_path).expect("Fail to open dump file");

    let init_option = matches
        .value_of("kvmi_socket")
        .map(|socket| DriverInitParam::KVMiSocket(socket.into()));
    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None, init_option);

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
                dump_file
                    .write_all(&buffer)
                    .expect("failed to write to file");
            }
            Err(_error) => (),
        }
    }

    println!("resuming the VM");
    drv.resume().expect("Failed to resume VM");
}
