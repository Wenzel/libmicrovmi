pub mod config;

use std::sync::{mpsc, Once};
use std::thread;
use std::time::Duration;

use config::TIMEOUT;

// to init env logger
static INIT: Once = Once::new();

pub fn run_test_generic<A, B, C>(setup: A, teardown: B, test: C) -> ()
where
    A: Send + 'static,
    A: FnOnce() -> (),
    B: Send + 'static,
    B: FnOnce() -> (),
    C: Send + 'static,
    C: FnOnce() -> (),
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
        let val = test();
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
