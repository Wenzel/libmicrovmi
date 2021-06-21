use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use clap::{App, Arg, ArgMatches};
use colored::*;

use microvmi::api::events::{CrType, EventType, InterceptType};
use microvmi::api::params::DriverInitParams;
use microvmi::api::Introspectable;
use utilities::Clappable;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.3")
        .author("Mathieu Tarral")
        .about("Watches control register VMI events")
        .args(DriverInitParams::to_clap_args().as_ref())
        .arg(
            Arg::with_name("register")
                .multiple(true)
                .takes_value(true)
                .short("r")
                .default_value("3")
                .help("control register to intercept. Possible values: [0 3 4]"),
        )
        .get_matches()
}

fn toggle_cr_intercepts(drv: &mut Box<dyn Introspectable>, vec_cr: &[CrType], enabled: bool) {
    drv.pause().expect("Failed to pause VM");

    for cr in vec_cr {
        let intercept = InterceptType::Cr(*cr);
        let status_str = if enabled { "Enabling" } else { "Disabling" };
        println!("{} intercept on {:?}", status_str, cr);
        //for vcpu in 0..drv.get_vcpu_count().unwrap() {
        let vcpu = 0;
        drv.toggle_intercept(vcpu, intercept, enabled)
            .unwrap_or_else(|_| panic!("Failed to enable {:?}", cr));
        //}
    }

    drv.resume().expect("Failed to resume VM");
}

fn get_cr(registers: Vec<&str>) -> Vec<CrType> {
    let mut vec_cr = Vec::new();
    for reg_str in registers {
        let cr = match reg_str {
            "0" => CrType::Cr0,
            "3" => CrType::Cr3,
            "4" => CrType::Cr4,
            x => panic!(
                "Provided register value \"{}\" is not a valid/interceptable control register.",
                x
            ),
        };
        vec_cr.push(cr);
    }
    vec_cr
}

fn main() {
    env_logger::init();

    let matches = parse_args();

    let registers: Vec<_> = matches.values_of("register").unwrap().collect();

    let vec_cr = get_cr(registers);

    // set CTRL-C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!("Initialize Libmicrovmi");
    let init_params = DriverInitParams::from_matches(&matches);
    let mut drv: Box<dyn Introspectable> =
        microvmi::init(None, Some(init_params)).expect("Failed to init libmicrovmi");

    // enable control register interception
    toggle_cr_intercepts(&mut drv, &vec_cr, true);

    println!("Listen for control register events...");
    // record elapsed time
    let start = Instant::now();
    // listen
    let mut i: u64 = 0;
    while running.load(Ordering::SeqCst) {
        let event = drv.listen(10).expect("Failed to listen for events");
        match event {
            Some(ev) => {
                let (cr_type, new, old) = match ev.kind {
                    EventType::Cr { cr_type, new, old } => (cr_type, new, old),
                    _ => panic!("not cr event"),
                };
                let cr_color = match cr_type {
                    CrType::Cr0 => "blue",
                    CrType::Cr3 => "green",
                    CrType::Cr4 => "red",
                };
                let ev_nb_output = format!("{}", i).cyan();
                let vcpu_output = format!("VCPU {}", ev.vcpu).yellow();
                let cr_output = format!("{:?}", cr_type).color(cr_color);
                println!(
                    "[{}] {} - {}:    old value: 0x{:x}    new value: 0x{:x}",
                    ev_nb_output, vcpu_output, cr_output, old, new
                );
                // drv.reply_event(ev, EventReplyType::Continue)
                //   .expect("Failed to send event reply");
                i += 1;
            }
            None => println!("No events yet..."),
        }
    }
    let duration = start.elapsed();

    // disable control register interception
    toggle_cr_intercepts(&mut drv, &vec_cr, false);

    let ev_per_sec = i as f64 / duration.as_secs_f64();
    println!(
        "Caught {} events in {:.2} seconds ({:.2} events/sec)",
        i,
        duration.as_secs_f64(),
        ev_per_sec
    );
}
