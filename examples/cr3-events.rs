use std::env;

use env_logger;

use microvmi::api::{CrType, Event, EventReplyType, EventType, Introspectable};

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <vm_name>", args[0]);
        return;
    }
    let domain_name = &args[1];

    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None);

    println!("Enable CR3 interception");
    // enable CR3 interception
    let empty_cr_enum = EventType::Cr {
        cr_type: CrType::Cr3,
        new: 0,
        old: 0,
    };
    drv.toggle_intercept(0, empty_cr_enum, true)
        .expect("Failed to enable CR3 interception");

    println!("Listen for CR3 events...");
    // listen
    for i in 1..100 {
        let event = drv.listen(1000).expect("Failed to listen for events");
        match event {
            Some(ev) => {
                println!("[{}] {:?}", i, ev);
                drv.reply_event(ev, EventReplyType::Continue);
            }
            None => println!("No events yet..."),
        }
    }

    println!("Disable CR3 interception");
    // disable CR3 interception
    drv.toggle_intercept(0, empty_cr_enum, true)
        .expect("Failed to enable CR3 interception");
}
