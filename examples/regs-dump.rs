use clap::{App, ArgMatches};

use microvmi::api::params::DriverInitParams;
use microvmi::api::Introspectable;

use utilities::Clappable;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .author("Mathieu Tarral")
        .about("Dumps the state of registers on VCPU 0")
        .args(DriverInitParams::to_clap_args().as_ref())
        .get_matches()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let matches = parse_args();
    let init_params = DriverInitParams::from_matches(&matches);
    let mut drv: Box<dyn Introspectable> = microvmi::init(None, Some(init_params))?;

    println!("pausing the VM");
    drv.pause()?;
    let dump = (|| -> Result<(), Box<dyn std::error::Error>> {
        let total_vcpu_count = drv.get_vcpu_count()?;
        for vcpu in 0..total_vcpu_count {
            println!("dumping registers on VCPU {}", vcpu);
            println!("{:#x?}", drv.read_registers(vcpu)?);
        }
        Ok(())
    })();

    println!("resuming the VM");
    let resume = drv.resume();
    dump?;
    resume?;
    Ok(())
}
