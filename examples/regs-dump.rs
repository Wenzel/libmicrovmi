use clap::{App, Arg, ArgMatches};

use microvmi::api::DriverInitParam;
use microvmi::Microvmi;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .author("Mathieu Tarral")
        .about("Dumps the state of registers on VCPU 0")
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

    let init_option = matches
        .value_of("kvmi_socket")
        .map(|socket| DriverInitParam::KVMiSocket(socket.into()));
    let mut drv =
        Microvmi::new(domain_name, None, init_option).expect("Failed to init libmicrovmi");

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
