use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use clap::{App, Arg, ArgMatches};
use colored::*;
use env_logger;

use microvmi::api::{DriverInitParam, EventReplyType, EventType, InterceptType, Introspectable};

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .about("Watches interrupt VMI events")
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

fn toggle_int3_interception(drv: &mut Box<dyn Introspectable>, enabled: bool) {
    drv.pause().expect("Failed to pause VM");

    let intercept = InterceptType::Breakpoint;
    let status_str = if enabled { "Enabling" } else { "Disabling" };
    println!("{} interrupt events", status_str);
    for vcpu in 0..drv.get_vcpu_count().unwrap() {
        drv.toggle_intercept(vcpu, intercept, enabled)
            .expect(&format!("Failed to enable interrupts"));
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
                let interrupt_output = format!("interrupt occurred!").color("blue");
                println!(
                    "[{}] {} - {}: gpa = 0x{:x}    insn_len = 0x{:x}",
                    ev_nb_output, vcpu_output, interrupt_output, gpa, insn_len
                );
                drv.reply_event(ev, EventReplyType::Continue)
                    .expect("Failed to send event reply");
                i = i + 1;
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
