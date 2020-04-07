use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use env_logger;

use microvmi::api::{CrType, EventReplyType, EventType, InterceptType, Introspectable};

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <vm_name>", args[0]);
        return;
    }
    let domain_name = &args[1];

    // set CTRL-C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);

    println!("Enable CR3 interception");
    drv.pause().expect("Failed to pause VM");

    // enable CR3 interception
    let inter_cr3 = InterceptType::Cr(CrType::Cr3);
    for vcpu in 0..drv.get_vcpu_count().unwrap() {
        drv.toggle_intercept(vcpu, inter_cr3, true)
            .expect("Failed to enable CR3 interception");
    }

    drv.resume().expect("Failed to resume VM");

    println!("Listen for CR3 events...");
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
                drv.reply_event(&ev, EventReplyType::Continue)
                    .expect("Failed to send event reply");
            }
            None => println!("No events yet..."),
        }
        i = i + 1;
    }
    let duration = start.elapsed();

    println!("Disable CR3 interception");
    drv.pause().expect("Failed to pause VM");

    // disable CR3 interception
    for vcpu in 0..drv.get_vcpu_count().unwrap() {
        drv.toggle_intercept(vcpu, inter_cr3, false)
            .expect("Failed to enable CR3 interception");
    }

    let ev_per_sec = i as f64 / duration.as_secs_f64();
    println!(
        "Caught {} events in {:.2} seconds ({:.2} events/sec)",
        i,
        duration.as_secs_f64(),
        ev_per_sec
    );
}
