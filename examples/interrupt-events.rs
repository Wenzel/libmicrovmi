use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use clap::{App, ArgMatches};
use colored::*;

use microvmi::api::params::DriverInitParams;
use microvmi::api::{EventReplyType, EventType, InterceptType, Introspectable};

use utilities::Clappable;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .about("Watches interrupt VMI events")
        .args(DriverInitParams::to_clap_args().as_ref())
        .get_matches()
}

fn toggle_int3_interception(drv: &mut Box<dyn Introspectable>, enabled: bool) {
    drv.pause().expect("Failed to pause VM");

    let intercept = InterceptType::Breakpoint;
    let status_str = if enabled { "Enabling" } else { "Disabling" };
    println!("{} interrupt events", status_str);
    for vcpu in 0..drv.get_vcpu_count().unwrap() {
        drv.toggle_intercept(vcpu, intercept, enabled)
            .expect("Failed to enable interrupts");
    }

    drv.resume().expect("Failed to resume VM");
}

fn main() {
    env_logger::init();

    let matches = parse_args();

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

    //Enable int3 interception
    toggle_int3_interception(&mut drv, true);

    println!("Listen for interrupt events...");
    // record elapsed time
    let start = Instant::now();
    // listen
    let mut i: u64 = 0;
    while running.load(Ordering::SeqCst) {
        let event = drv.listen(1000).expect("Failed to listen for events");
        match event {
            Some(ev) => {
                let (gpa, insn_len) = match ev.kind {
                    EventType::Breakpoint { gpa, insn_len } => (gpa, insn_len),
                    _ => panic!("Not interrupt event"),
                };
                let ev_nb_output = format!("{}", i).cyan();
                let vcpu_output = format!("VCPU {}", ev.vcpu).yellow();
                let interrupt_output = "interrupt occurred!".color("blue");
                println!(
                    "[{}] {} - {}: gpa = 0x{:x}    insn_len = 0x{:x}",
                    ev_nb_output, vcpu_output, interrupt_output, gpa, insn_len
                );
                drv.reply_event(ev, EventReplyType::Continue)
                    .expect("Failed to send event reply");
                i += 1;
            }
            None => println!("No events yet..."),
        }
    }
    let duration = start.elapsed();

    //disable int3 interception
    toggle_int3_interception(&mut drv, false);

    let ev_per_sec = i as f64 / duration.as_secs_f64();
    println!(
        "Caught {} events in {:.2} seconds ({:.2} events/sec)",
        i,
        duration.as_secs_f64(),
        ev_per_sec
    );
}
