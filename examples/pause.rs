use std::{thread, time};

use clap::{App, Arg, ArgMatches};
use env_logger;

use microvmi::api::{DriverInitParam, Introspectable};

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .author("Mathieu Tarral")
        .about("Pauses and resumes the VM")
        .arg(Arg::with_name("vm_name").index(1).required(true))
        .arg(
            Arg::with_name("timeout")
                .takes_value(true)
                .default_value("5")
                .help("pause the VM during timeout seconds"),
        )
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
    let timeout = matches.value_of("timeout").unwrap().parse::<u64>().unwrap();

    let init_option = matches
        .value_of("kvmi_socket")
        .map(|socket| DriverInitParam::KVMiSocket(socket.into()));
    let mut drv: Box<dyn Introspectable> =
        microvmi::init(domain_name, None, init_option).expect("Failed to init libmicrovmi");

    println!("pausing VM for {} seconds", timeout);
    drv.pause().expect("Failed to pause VM");

    let duration = time::Duration::new(timeout, 0);
    thread::sleep(duration);

    println!("resuming VM");
    drv.resume().expect("Failed to resume VM");
}
