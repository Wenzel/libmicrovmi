use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use clap::{App, Arg, ArgMatches};
use colored::*;
use env_logger;

use microvmi::api::*;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .about("Watches singlestep VMI events")
        .arg(Arg::with_name("vm_name").index(1).required(true))
        .get_matches()
}

fn toggle_singlestep_interception(drv: &mut Box<dyn Introspectable>, enabled: bool) {
    drv.pause().expect("Failed to pause VM");

    let intercept = InterceptType::Singlestep;
    let status_str = if enabled { "Enabling" } else { "Disabling" };
    println!("{} singlestep events", status_str);
    for vcpu in 0..1 {
        drv.toggle_intercept(vcpu, intercept, enabled)
            .expect(&format!("Failed to enable singlestep"));
    }

    drv.resume().expect("Failed to resume VM");
}

fn main() {
    env_logger::init();

    let matches = parse_args();

    let domain_name = matches.value_of("vm_name").unwrap();

    let init_option = matches
        .value_of("kvmi_socket")
        .map(|socket| DriverInitParam::KVMiSocket(socket.into()));
    // set CTRL-C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!("Initialize Libmicrovmi");
    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None, init_option);

    //Enable singlestep interception
    toggle_singlestep_interception(&mut drv, true);

    println!("Listen for singlestep events...");
    // record elapsed time
    let start = Instant::now();
    // listen
    let mut i: u64 = 0;
    while running.load(Ordering::SeqCst) {
        let event = drv.listen(1000).expect("Failed to listen for events");
        match event {
            Some(ev) => {
                let gpa = match ev.kind {
                    EventType::Singlestep { gpa } => (gpa),
                    _ => panic!("Not singlestep event"),
                };
                let ev_nb_output = format!("{}", i).cyan();
                let vcpu_output = format!("VCPU {}", ev.vcpu).yellow();
                let singlestep_output = format!("singlestep occurred!").color("blue");
                println!(
                    "[{}] {} - {}: gpa = 0x{:x} ",
                    ev_nb_output, vcpu_output, singlestep_output, gpa
                );
                //drv.reply_event(ev, EventReplyType::Continue)
                //  .expect("Failed to send event reply");
                i = i + 1;
            }
            None => println!("No events yet..."),
        }
    }
    let duration = start.elapsed();

    //disable singlestep interception
    toggle_singlestep_interception(&mut drv, false);

    let ev_per_sec = i as f64 / duration.as_secs_f64();
    println!(
        "Caught {} events in {:.2} seconds ({:.2} events/sec)",
        i,
        duration.as_secs_f64(),
        ev_per_sec
    );
}
