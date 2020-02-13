use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use env_logger;

use microvmi::api::{CrType, EventReplyType, EventType, InterceptType, Introspectable};
use microvmi::Ev;

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

    let mut drv: Box<dyn Introspectable<DriverEvent=Ev>> = microvmi::init(domain_name, None);

    println!("Enable CR3 interception");
    // enable CR3 interception
    let inter_cr3 = InterceptType::Cr(CrType::Cr3);
    drv.toggle_intercept(0, inter_cr3, true)
        .expect("Failed to enable CR3 interception");

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
                println!("[{}] CR3: 0x{:x}", i, new);
                drv.reply_event(&ev, EventReplyType::Continue).unwrap();
            }
            None => println!("No events yet..."),
        }
        i = i + 1;
    }
    let duration = start.elapsed();

    println!("Disable CR3 interception");
    // disable CR3 interception
    drv.toggle_intercept(0, inter_cr3, false)
        .expect("Failed to enable CR3 interception");

    println!("Catched {} events/sec", i / duration.as_secs());
}
