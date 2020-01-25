use std::env;

use env_logger;

use microvmi::api::EventType;
use microvmi::api::Introspectable;

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
    drv.pause().expect("Failed to pause VM");

    // enable CR3 interception
    drv.toggle_intercept(0, EventType::CR3, true)
        .expect("Failed to enable CR3 interception");

    drv.resume().expect("Failed to resume VM");

    println!("Listen for CR3 events...");
    // listen
    for i in 1..100 {
        let event = drv.listen(1000);
        println!("[{}] {:?}", i, event);
    }

    println!("Disable CR3 interception");
    drv.pause().expect("Failed to pause VM");

    // disable CR3 interception
    drv.toggle_intercept(0, EventType::CR3, true)
        .expect("Failed to enable CR3 interception");

    drv.resume().expect("Failed to resume VM");
}
