use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use clap::{App, Arg, ArgMatches};
use colored::*;
use env_logger;

use microvmi::api::{MsrType, EventReplyType, EventType, InterceptType, Introspectable};

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.3")
        .author("Mathieu Tarral")
        .about("Watches control register VMI events")
        .arg(Arg::with_name("vm_name").index(1).required(true))
        .arg(
            Arg::with_name("register")
                .multiple(true)
                .takes_value(true)
                .short("r")
                .default_value("3")
                .help("MSR to intercept. Possible values: [0 1 2 3 4 5]"),
        )
        .get_matches()
}

fn toggle_msr_intercepts(drv: &mut Box<dyn Introspectable>, vec_msr: &Vec<MsrType>, enabled: bool) {
    drv.pause().expect("Failed to pause VM");

    for msr in vec_msr {
        let intercept = InterceptType::Msr(*msr);
        let status_str = if enabled { "Enabling" } else { "Disabling" };
        println!("{} intercept on {:?}", status_str, msr);
        for vcpu in 0..drv.get_vcpu_count().unwrap() {
            drv.toggle_intercept(vcpu, intercept, enabled)
                .expect(&format!("Failed to enable {:?}", msr));
        }
    }

    drv.resume().expect("Failed to resume VM");
}

fn main() {
    env_logger::init();

    let matches = parse_args();

    let domain_name = matches.value_of("vm_name").unwrap();
    let registers: Vec<_> = matches.values_of("register").unwrap().collect();

    // check parameters
    let mut vec_msr = Vec::new();
    for reg_str in registers {
        let msr = match reg_str {
            "0" => MsrType::Sysenter_cs,
            "1" => MsrType::Sysenter_esp,
            "2" => MsrType::Sysenter_eip,
            "3" => MsrType::Msr_star,
            "4" => MsrType::Msr_lstar,
            "5" => MsrType::Msr_efer,
            x => panic!(
                "Provided register value \"{}\" is not a valid/interceptable msr register.",
                x
            ),
        };
        vec_msr.push(msr);
    }

    // set CTRL-C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!("Initialize Libmicrovmi");
    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);

    // enable control register interception
    toggle_msr_intercepts(&mut drv, &vec_msr, true);

    println!("Listen for MSR events...");
    // record elapsed time
    let start = Instant::now();
    // listen
    let mut i: u64 = 0;
    while running.load(Ordering::SeqCst) {
        let event = drv.listen(1000).expect("Failed to listen for events");
        match event {
            Some(ev) => {
                let (msr_type, new) = match ev.kind {
                    EventType::Msr {
                        msr_type,
                        new,
                        old: _,
                    } => (msr_type, new),
                    _ => (MsrType::Sysenter_cs,0),
                };
                let msr_color = match msr_type {
                    MsrType::Sysenter_cs => "blue",
                    MsrType::Sysenter_esp => "black",
                    MsrType::Sysenter_eip => "green",
                    MsrType::Msr_star => "red",
                    MsrType::Msr_lstar => "yellow",
                    MsrType::Msr_efer => "white",
                };
                let ev_nb_output = format!("{}", i).cyan();
                let vcpu_output = format!("VCPU {}", ev.vcpu).yellow();
                let msr_output = format!("{:?}", msr_type).color(msr_color);
                // let output = format!("[{}] VCPU {} - {:?}: 0x{:x}", i, ev.vcpu, cr_type, new);
                println!(
                    "[{}] {} - {}: 0x{:x}",
                    ev_nb_output, vcpu_output, msr_output, new
                );
                drv.reply_event(ev, EventReplyType::Continue)
                    .expect("Failed to send event reply");
                i = i + 1;
            }
            None => println!("No events yet..."),
        }
    }
    let duration = start.elapsed();

    // disable control register interception
    toggle_msr_intercepts(&mut drv, &vec_msr, false);

    let ev_per_sec = i as f64 / duration.as_secs_f64();
    println!(
        "Caught {} events in {:.2} seconds ({:.2} events/sec)",
        i,
        duration.as_secs_f64(),
        ev_per_sec
    );
}
