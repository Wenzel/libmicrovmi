use clap::{App, Arg, ArgMatches};
use std::{thread, time};

use microvmi::api::params::DriverInitParams;
use microvmi::api::Introspectable;

use utilities::Clappable;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .author("Mathieu Tarral")
        .about("Pauses and resumes the VM")
        .arg(
            Arg::with_name("timeout")
                .takes_value(true)
                .default_value("5")
                .help("pause the VM during timeout seconds"),
        )
        .args(DriverInitParams::to_clap_args().as_ref())
        .get_matches()
}

fn main() {
    env_logger::init();

    let matches = parse_args();
    let timeout = matches.value_of("timeout").unwrap().parse::<u64>().unwrap();

    // get driver params
    let init_params = DriverInitParams::from_matches(&matches);
    let mut drv: Box<dyn Introspectable> =
        microvmi::init(None, Some(init_params)).expect("Failed to init libmicrovmi");

    println!("pausing VM for {} seconds", timeout);
    drv.pause().expect("Failed to pause VM");

    let duration = time::Duration::new(timeout, 0);
    thread::sleep(duration);

    println!("resuming VM");
    drv.resume().expect("Failed to resume VM");
}
