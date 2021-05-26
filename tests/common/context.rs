/* This module defines the run_test function to execute tests with a custom setup
teardown code.
 */

use std::sync::{mpsc, Once};
use std::thread;
use std::time::Duration;

use crate::common::kvm::KVM;

use super::config::TIMEOUT;
use microvmi::api::Introspectable;

// to init env logger
static INIT: Once = Once::new();

fn run_test_generic<A, B, C, D>(setup: A, teardown: B, init_driver: C, test: D) -> ()
where
    A: Send,
    A: FnOnce() -> (),
    B: Send,
    B: FnOnce() -> (),
    C: Send + 'static,
    C: FnOnce() -> Box<dyn Introspectable>,
    D: Send + 'static,
    D: FnOnce(Box<dyn Introspectable>) -> (),
{
    // init env_logger if necessary
    INIT.call_once(|| {
        env_logger::builder().is_test(true).init();
    });
    // setup before test
    setup();

    // setup test execution in a thread
    let (done_tx, done_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let driver = init_driver();
        let val = test(driver);
        done_tx.send(()).expect("Unable to send completion signal");
        val
    });

    // wait for test to complete until timeout
    let timeout = Duration::from_secs(TIMEOUT);
    let res = done_rx.recv_timeout(timeout).map(|_| handle.join());
    // cleanup test
    teardown();
    // check results
    res.expect("Test timeout").expect("Test panicked");
}
//
// // define run_test with setup / teardown
// pub fn run_test<T>(test: T) -> ()
// where
//     T: Send + 'static,
//     T: FnOnce(Box<dyn Introspectable>) -> (),
// {
//     run_test_generic(setup_test, teardown_test, init_driver, test)
// }

pub trait Context: Send {
    fn setup(&self) -> ();
    fn init_driver(&self) -> Box<dyn Introspectable>;
    fn teardown(&self) -> ();
}

pub fn init_context() -> Box<dyn Context> {
    if cfg!(feature = "kvm") {
        Box::new(KVM {})
    } else {
        panic!("Integration tests need to be run with a specific driver enabled")
    }
}
