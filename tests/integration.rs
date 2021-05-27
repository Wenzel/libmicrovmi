use std::io;
use std::panic::catch_unwind;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use colored::*;
use env_logger;

mod common;
mod tests;
use common::config::TIMEOUT;
use common::context::init_context;
use std::io::Write;
use tests::IntegrationTest;

fn main() {
    // init logger
    env_logger::builder().is_test(true).init();
    // for each test
    for test in inventory::iter::<IntegrationTest> {
        print!("Test {} ... ", test.name);
        io::stdout().flush().expect("Failed to flush stdout");
        // get setup / teardown context
        let ctx = init_context();
        // setup environment before test
        ctx.setup();

        // configure test execution in a thread
        let (done_tx, done_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let val = catch_unwind(|| {
                // TODO: remove this
                let ctx2 = init_context();
                let driver = ctx2.init_driver();
                (test.test_fn)(driver)
            });
            done_tx.send(()).expect("Unable to send completion signal");
            val
        });

        // wait for test to complete until timeout
        let timeout = Duration::from_secs(TIMEOUT);
        let timeout_result = done_rx.recv_timeout(timeout).map(|_| handle.join());
        // cleanup test environment
        ctx.teardown();
        // check results
        match timeout_result {
            Err(_) => println!("{}: {}", "Failed".red(), "Timeout".yellow()),
            Ok(join_result) => match join_result {
                Err(cause) => println!(
                    "{}: test runner failed to join thread: {:?}",
                    "Failed".red(),
                    cause
                ),
                Ok(catch_unwind_result) => match catch_unwind_result {
                    Err(_) => println!("{}", "Failed".red()),
                    Ok(_) => println!("{}", "ok".green()),
                },
            },
        }
    }
}
