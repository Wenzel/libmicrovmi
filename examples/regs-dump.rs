use clap::{App, Arg, ArgMatches};

use microvmi::api::params::DriverInitParams;
use microvmi::api::Introspectable;

use utilities::Clappable;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .author("Mathieu Tarral")
        .about("Dumps the state of registers on VCPU 0")
        .arg(Arg::with_name("vm_name").index(1).required(true))
        .args(DriverInitParams::to_clap_args().as_ref())
        .get_matches()
}

fn main() {
    env_logger::init();

    let matches = parse_args();
    let init_params = DriverInitParams::from_matches(&matches);
    let mut drv: Box<dyn Introspectable> =
        microvmi::init(None, Some(init_params)).expect("Failed to init libmicrovmi");

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
