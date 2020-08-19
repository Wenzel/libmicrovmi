use clap::{App, Arg, ArgMatches};
use colored::*;
use env_logger;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use microvmi::api::{
    Access, DriverInitParam, EventReplyType, EventType, InterceptType, Introspectable,
};

const PAGE_SIZE: usize = 4096;

fn parse_args() -> ArgMatches<'static> {
    App::new(file!())
        .version("0.1")
        .about("Watches memory VMI events")
        .arg(Arg::with_name("vm_name").index(1).required(true))
        .get_matches()
}

fn toggle_pf_intercept(drv: &mut Box<dyn Introspectable>, enabled: bool) {
    drv.pause().expect("Failed to pause VM");

    let intercept = InterceptType::Pagefault;
    let status_str = if enabled { "Enabling" } else { "Disabling" };
    println!("{} memory events", status_str);
    for vcpu in 0..drv.get_vcpu_count().unwrap() {
        drv.toggle_intercept(vcpu, intercept, enabled)
            .expect(&format!("Failed to enable page faults"));
    }

    drv.resume().expect("Failed to resume VM");
}

fn main() {
    env_logger::init();

    let matches = parse_args();

    let domain_name = matches.value_of("vm_name").unwrap();

    // set CTRL-C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!("Initialize Libmicrovmi");
    let init_option = matches
        .value_of("kvmi_socket")
        .map(|socket| DriverInitParam::KVMiSocket(socket.into()));
    let mut drv: Box<dyn Introspectable> = microvmi::init(domain_name, None, init_option);
    println!("Listen for memory events...");
    // record elapsed time
    let start = Instant::now();
    toggle_pf_intercept(&mut drv, true);
    let mut i: u64 = 0;

    //Code snippet to get page fault
    let max_addr = drv.get_max_physical_addr().unwrap();

    for cur_addr in (0..max_addr).step_by(PAGE_SIZE) {
        let mut access: Access = drv.get_page_access(cur_addr).unwrap();
        access &= !Access::X;
        drv.set_page_access(cur_addr, access)
            .expect("failed to set page access");
    }

    while running.load(Ordering::SeqCst) {
        let event = drv.listen(1000).expect("Failed to listen for events");
        match event {
            Some(ev) => {
                let (gva, gpa, pf_access) = match ev.kind {
                    EventType::Pagefault { gva, gpa, access } => (gva, gpa, access),
                    _ => panic!("Not pf event"),
                };
                let ev_nb_output = format!("{}", i).cyan();
                let vcpu_output = format!("VCPU {}", ev.vcpu).yellow();
                let pagefault_output = format!("pagefault occurred!").color("blue");
                println!(
                    "[{}] {} - {}:   gva = 0x{:x}    gpa = 0x{:x}    access = {:?} ",
                    ev_nb_output, vcpu_output, pagefault_output, gva, gpa, pf_access
                );
                let mut page_access = drv.get_page_access(gpa).expect("Failed to get page access");
                //setting the access bits in the page due to which page fault occurred
                page_access |= pf_access;
                drv.set_page_access(gpa, page_access)
                    .expect("Failed to set page access");
                drv.reply_event(ev, EventReplyType::Continue)
                    .expect("Failed to send event reply");
                i = i + 1;
            }
            None => println!("No events yet..."),
        }
    }
    let duration = start.elapsed();
    toggle_pf_intercept(&mut drv, false);

    let ev_per_sec = i as f64 / duration.as_secs_f64();
    println!(
        "Caught {} events in {:.2} seconds ({:.2} events/sec)",
        i,
        duration.as_secs_f64(),
        ev_per_sec
    );
}
