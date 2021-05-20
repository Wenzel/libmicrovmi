use env_logger;
use log::debug;
use std::panic;
use std::process::Command;
use std::sync::Once;

// to init env logger
static INIT: Once = Once::new();

static VM_NAME: &str = "winxp";
static VIRSH_URI: &str = "qemu:///system";
static KVMI_SOCKET: &str = "/tmp/introspector";

fn run_test<T>(test: T) -> ()
where
    T: FnOnce() -> () + panic::UnwindSafe,
{
    INIT.call_once(|| {
        env_logger::builder().is_test(true).init();
    });
    setup_test();
    let result = panic::catch_unwind(|| test());
    teardown_test();
    assert!(result.is_ok())
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
        .status()
        .expect("Failed to start virsh")
        .success()
        .then(|| 0)
        .expect("Failed to run virsh destroy");
}

#[cfg(feature = "kvm")]
mod tests {
    use super::*;
    use microvmi::api::{DriverInitParam, DriverType, Introspectable};
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

    #[test]
    fn test_pause() {
        run_test(|| {
            let mut drv = init_driver();
            drv.pause().unwrap();
        })
    }
}
