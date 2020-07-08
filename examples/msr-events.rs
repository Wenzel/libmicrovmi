use clap::{App, Arg, ArgMatches};
use colored::*;
use env_logger;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::u32;

use microvmi::api::{EventReplyType, EventType, InterceptType, Introspectable};

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .about("Watches msr register VMI events")
        .arg(Arg::with_name("vm_name").index(1).required(true))
        .arg(
            Arg::with_name("register")
                .multiple(true)
                .takes_value(true)
                .short("r")
                .default_value("0x174"),
        )
        .get_matches()
}

fn toggle_msr_intercepts(drv: &mut Box<dyn Introspectable>, vec_msr: &Vec<u32>, enabled: bool) {
    drv.pause().expect("Failed to pause VM");

    for msr in vec_msr {
        let intercept = InterceptType::Msr(*msr);
        let status_str = if enabled { "Enabling" } else { "Disabling" };
        println!("{} intercept on 0x{:x}", status_str, msr);
        for vcpu in 0..drv.get_vcpu_count().unwrap() {
            drv.toggle_intercept(vcpu, intercept, enabled)
                .expect(&format!("Failed to enable 0x{:x}", msr));
        }
    }

    drv.resume().expect("Failed to resume VM");
}

fn get_msr(registers: Vec<&str>) -> Vec<u32> {
    let mut vec_msr = Vec::new();
    for reg_str in registers {
        let msr = match reg_str {
            "sysentercs" => 0x174 as u32,
            "sysenteresp" => 0x175 as u32,
            "sysentereip" => 0x176 as u32,
            "star" => 0xc0000080 as u32,
            "lstar" => 0xc0000081 as u32,
            "efer" => 0xc0000082 as u32,
            x => panic!(
                "Provided register value \"{}\" is not a valid/interceptable msr register.",
                x
            ),
        };
        vec_msr.push(msr);
    }
    vec_msr
}

fn main() {
    env_logger::init();

    let matches = parse_args();

    let domain_name = matches.value_of("vm_name").unwrap();
    let registers: Vec<_> = matches.values_of("register").unwrap().collect();

    let vec_msr = get_msr(registers);

    // set CTRL-C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!("Initialize Libmicrovmi");
    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);

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
                let (msr_type, new, old) = match ev.kind {
                    EventType::Msr { msr_type, new, old } => (msr_type, new, old),
                    _ => panic!("not msr event"),
                };
                let msr_color = "blue";
                let ev_nb_output = format!("{}", i).cyan();
                let vcpu_output = format!("VCPU {}", ev.vcpu).yellow();
                let msr_output = format!("0x{:x}", msr_type).color(msr_color);
                println!(
                    "[{}] {} - {}: old value: 0x{:x} new value: 0x{:x}",
                    ev_nb_output, vcpu_output, msr_output, old, new
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
