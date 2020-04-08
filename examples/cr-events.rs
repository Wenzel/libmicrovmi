use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use env_logger;
use clap::{Arg, App};

use microvmi::api::{CrType, EventReplyType, EventType, InterceptType, Introspectable};

fn main() {
    env_logger::init();

    let matches = App::new(file!())
        .version("0.2")
        .author("Mathieu Tarral")
        .about("Watches control register VMI events")
        .arg(
            Arg::with_name("vm_name")
                .index(1)
                .required(true)
        )
        .arg(
            Arg::with_name("register")
                .multiple(true)
                .takes_value(true)
                .short("r")
                .default_value("3")
                .help("control register to intercept. Possible values: [0 3 4]")
        )
        .get_matches();

    let domain_name = matches.value_of("vm_name").unwrap();
    let registers: Vec<_> = matches.values_of("register").unwrap().collect();

    // check parameters
    let mut vec_cr = Vec::new();
    for reg_str in registers {
        let cr = match reg_str {
            "0" => CrType::Cr0,
            "3" => CrType::Cr3,
            "4" => CrType::Cr4,
            x => panic!("Provided register value \"{}\" is not a valid/interceptable control register.", x)
        };
        vec_cr.push(cr);
    }

    // set CTRL-C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);

    drv.pause().expect("Failed to pause VM");

    // enable control register interception
    for cr in &vec_cr {
        let intercept = InterceptType::Cr(*cr);
        println!("Enabling intercept on {:?}", cr);
        for vcpu in 0..drv.get_vcpu_count().unwrap() {
            drv.toggle_intercept(vcpu, intercept, true)
                .expect(&format!("Failed to enable {:?}", cr));
        }
    }

    drv.resume().expect("Failed to resume VM");

    println!("Listen for control register events...");
    // record elapsed time
    let start = Instant::now();
    // listen
    let mut i: u64 = 1;
    while running.load(Ordering::SeqCst) {
        let event = drv.listen(1000).expect("Failed to listen for events");
        match event {
            Some(ev) => {
                let new = match ev.kind {
                    EventType::Cr {
                        cr_type: _,
                        new,
                        old: _,
                    } => new,
                };
                println!("[{}] VCPU {} - CR3: 0x{:x}", i, ev.vcpu, new);
                drv.reply_event(ev, EventReplyType::Continue)
                    .expect("Failed to send event reply");
            }
            None => println!("No events yet..."),
        }
        i = i + 1;
    }
    let duration = start.elapsed();

    drv.pause().expect("Failed to pause VM");

    // disable control register interception
    for cr in &vec_cr {
        let intercept = InterceptType::Cr(*cr);
        println!("Disbaling intercept of {:?}", cr);
        for vcpu in 0..drv.get_vcpu_count().unwrap() {
            drv.toggle_intercept(vcpu, intercept, false)
                .expect("Failed to disable control register interception");
        }
    }

    let ev_per_sec = i as f64 / duration.as_secs_f64();
    println!(
        "Caught {} events in {:.2} seconds ({:.2} events/sec)",
        i,
        duration.as_secs_f64(),
        ev_per_sec
    );
}
