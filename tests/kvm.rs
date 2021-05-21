mod config;

use env_logger;
use log::debug;
use std::mem;
use std::panic;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Once};
use std::thread;
use std::time::Duration;

use config::{KVMI_SOCKET, TIMEOUT, VIRSH_URI, VM_NAME};

// to init env logger
static INIT: Once = Once::new();

fn run_test<T>(test: T) -> ()
where
    T: Send + 'static,
    T: FnOnce() -> (),
{
    // init env_logger if necessary
    INIT.call_once(|| {
        env_logger::builder().is_test(true).init();
    });
    // setup before test
    setup_test();

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
    teardown_test();
    // check results
    res.expect("Test timeout").expect("Test panicked");
}

/// restore VM state from internal QEMU snapshot
fn setup_test() {
    debug!("setup test");
    Command::new("virsh")
        .arg(format!("--connect={}", VIRSH_URI))
        .arg("snapshot-revert")
        .arg(VM_NAME)
        .arg("--current")
        .arg("--running")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to start virsh")
        .success()
        .then(|| 0)
        .expect("Failed to run virsh snapshot-revert");
}

/// shutdown VM
fn teardown_test() {
    debug!("teardown test");
    Command::new("virsh")
        .arg(format!("--connect={}", VIRSH_URI))
        .arg("destroy")
        .arg(VM_NAME)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to start virsh")
        .success()
        .then(|| 0)
        .expect("Failed to run virsh destroy");
}

#[cfg(feature = "kvm")]
mod tests {
    use super::*;
    use microvmi::api::{
        CrType, DriverInitParam, DriverType, EventReplyType, EventType, InterceptType,
        Introspectable,
    };
    use microvmi::init;

    fn init_driver() -> Box<dyn Introspectable> {
        init(
            VM_NAME,
            Some(DriverType::KVM),
            Some(DriverInitParam::KVMiSocket(String::from(KVMI_SOCKET))),
        )
        .expect("Failed to init libmicrovmi")
    }

    #[test]
    fn test_init_driver() {
        run_test(|| {
            init_driver();
        })
    }

    // TODO: this test timeout, and makes intercept_cr3 tests timeout as well
    // #[test]
    // fn test_init_driver_twice() {
    //     run_test(|| {
    //         let drv = init_driver();
    //         mem::drop(drv);
    //         let _drv = init_driver();
    //     })
    // }

    #[test]
    fn test_pause() {
        run_test(|| {
            let mut drv = init_driver();
            drv.pause().unwrap();
        })
    }

    #[test]
    fn test_double_pause() {
        run_test(|| {
            let mut drv = init_driver();
            drv.pause().unwrap();
            drv.pause().unwrap();
        })
    }

    #[test]
    fn test_double_resume() {
        run_test(|| {
            let mut drv = init_driver();
            drv.resume().unwrap();
            drv.resume().unwrap();
        })
    }

    #[test]
    fn test_pause_resume() {
        run_test(|| {
            let mut drv = init_driver();
            drv.pause().unwrap();
            drv.resume().unwrap();
        })
    }

    #[test]
    fn test_multiple_pause_resume() {
        run_test(|| {
            let mut drv = init_driver();
            for _ in 0..50 {
                drv.pause().unwrap();
                drv.resume().unwrap();
            }
        })
    }

    #[test]
    fn test_intercept_cr3_one() {
        run_test(|| {
            let mut drv = init_driver();
            for vcpu in 0..drv.get_vcpu_count().unwrap() - 1 {
                drv.toggle_intercept(vcpu, InterceptType::Cr(CrType::Cr3), true)
                    .expect("Failed to toggle CR3 intercept");
            }
            let event = drv.listen(5000).unwrap().unwrap();
            assert!(matches!(
                event.kind,
                EventType::Cr {
                    cr_type: CrType::Cr3,
                    ..
                }
            ));
        })
    }

    #[test]
    fn test_intercept_cr3_multiple() {
        run_test(|| {
            let mut drv = init_driver();
            for vcpu in 0..drv.get_vcpu_count().unwrap() - 1 {
                drv.toggle_intercept(vcpu, InterceptType::Cr(CrType::Cr3), true)
                    .expect("Failed to toggle CR3 intercept");
            }
            for _ in 0..10 {
                let event = drv.listen(5000).unwrap().unwrap();
                match event.kind {
                    EventType::Cr { cr_type, .. } => {
                        assert_eq!(cr_type, CrType::Cr3);
                        drv.reply_event(event, EventReplyType::Continue)
                            .expect("Failed to send event reply");
                    }
                    _ => panic!("Failed"),
                }
            }
        })
    }
}