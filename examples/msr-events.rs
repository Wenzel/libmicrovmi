use clap::{App, Arg, ArgMatches};
use colored::*;
use env_logger;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::u32;

use microvmi::api::{DriverInitParam, EventReplyType, EventType, InterceptType, Introspectable};

// default set of MSRs to be intercepted
const DEFAULT_MSR: [u32; 6] = [0x174, 0x175, 0x176, 0xc0000080, 0xc0000081, 0xc0000082];

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.2")
        .about(
            "Watches MSR register VMI events\n\
                - MSR_IA32_SYSENTER_CS:   0x174\n\
                - MSR_IA32_SYSENTER_ESP:  0x175\n\
                - MSR_IA32_SYSENTER_EIP:  0x176\n\
                - MSR_EFER:               0xc0000080\n\
                - MSR_STAR:               0xc0000081\n\
                - MSR_LSTAR:              0xc0000082\
                ",
        )
        .arg(
            Arg::with_name("vm_name")
                .help("VM name")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("MSR index")
                .help("Specific set of MSRs to be intercepted")
                .multiple(true)
                .takes_value(true)
                .short("r"),
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

fn main() {
    env_logger::init();

    let matches = parse_args();

    let domain_name = matches.value_of("vm_name").unwrap();
    let mut registers: Vec<u32> = matches
        .values_of("register")
        .unwrap()
        .map(|s| {
            s.parse::<u32>()
                .expect("Unable to convert MSR index to u32")
        })
        .collect();
    if registers.is_empty() {
        // use default set
        registers = DEFAULT_MSR.to_vec();
    }

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

    toggle_msr_intercepts(&mut drv, &registers, true);

    println!("Listen for MSR events...");
    // record elapsed time
    let start = Instant::now();
    // listen
    let mut i: u64 = 0;
    while running.load(Ordering::SeqCst) {
        let event = drv.listen(1000).expect("Failed to listen for events");
        match event {
            Some(ev) => {
                let (msr_type, value) = match ev.kind {
                    EventType::Msr { msr_type, value } => (msr_type, value),
                    _ => panic!("not msr event"),
                };
                let msr_color = "blue";
                let ev_nb_output = format!("{}", i).cyan();
                let vcpu_output = format!("VCPU {}", ev.vcpu).yellow();
                let msr_output = format!("0x{:x}", msr_type).color(msr_color);
                println!(
                    "[{}] {} - {}: new value: 0x{:x}",
                    ev_nb_output, vcpu_output, msr_output, value,
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
    toggle_msr_intercepts(&mut drv, &registers, false);

    let ev_per_sec = i as f64 / duration.as_secs_f64();
    println!(
        "Caught {} events in {:.2} seconds ({:.2} events/sec)",
        i,
        duration.as_secs_f64(),
        ev_per_sec
    );
}
