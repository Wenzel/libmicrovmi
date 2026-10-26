use std::io;
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use colored::*;

use common::context::init_context;
use common::filter::is_selected;
use tests::IntegrationTest;

mod common;
mod tests;

fn main() {
    #[cfg(all(feature = "virtualbox-vmi", not(feature = "kvm")))]
    if std::env::var_os("MICROVMI_VBOXVMI_PAUSE_CHILD").is_some() {
        tests::virtualbox::vboxvmi_pause_child();
    }

    #[cfg(all(feature = "virtualbox-vmi", not(feature = "kvm")))]
    if std::env::var_os("MICROVMI_VBOXVMI_PARKED_CHILD").is_some() {
        tests::virtualbox::vboxvmi_parked_child();
    }

    #[cfg(all(feature = "virtualbox-vmi", not(feature = "kvm")))]
    if std::env::var_os("MICROVMI_VBOXVMI_HIDDEN_CHILD").is_some() {
        tests::virtualbox::vboxvmi_hidden_child();
    }

    // init logger
    env_logger::builder().is_test(true).init();
    // run only the tests whose name contains MICROVMI_TEST_FILTER
    let filter = std::env::var("MICROVMI_TEST_FILTER").ok();
    let (mut ran, mut failed) = (0, 0);
    // for each test
    for test in inventory::iter::<IntegrationTest> {
        if !is_selected(test.name, filter.as_deref()) {
            continue;
        }
        ran += 1;
        print!("Test {} ... ", test.name);
        io::stdout().flush().expect("Failed to flush stdout");
        // get setup / teardown context
        let ctx = init_context();
        // setup environment before test; on failure, teardown still restores it
        if catch_unwind(AssertUnwindSafe(|| ctx.setup())).is_err() {
            ctx.teardown();
            println!("{}: setup", "Failed".red());
            failed += 1;
            continue;
        }

        // before the test starts: VirtualBox's timeout queries the VM, racing the test's own sessions
        let timeout = std::env::var("MICROVMI_TEST_TIMEOUT").map_or_else(
            |_| ctx.timeout(),
            |t| Duration::from_secs(t.parse().expect("MICROVMI_TEST_TIMEOUT: seconds")),
        );

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
        let timeout_result = done_rx.recv_timeout(timeout).map(|_| handle.join());
        // cleanup test environment
        ctx.teardown();
        // check results
        let ok = matches!(timeout_result, Ok(Ok(Ok(_))));
        failed += usize::from(!ok);
        match timeout_result {
            Err(_) => {
                // the test thread can't be stopped and may still use the driver: stop here
                println!("{}: {}, aborting", "Failed".red(), "Timeout".yellow());
                std::process::exit(1);
            }
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
    if ran == 0 {
        println!("no test matches MICROVMI_TEST_FILTER={:?}", filter);
        std::process::exit(1);
    }
    if failed > 0 {
        println!("{} test(s) failed", failed);
        std::process::exit(1);
    }
}
