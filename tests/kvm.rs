use env_logger;
use log::{debug, info};
use std::panic;
use std::process::Command;

static VM_NAME: &str = "winxp";

fn run_test<T>(test: T) -> ()
where
    T: FnOnce() -> () + panic::UnwindSafe,
{
    env_logger::builder().is_test(true).try_init().unwrap();
    setup_test();
    let result = panic::catch_unwind(|| test());
    teardown_test();
    assert!(result.is_ok())
}

/// restore VM state from internal QEMU snapshot
fn setup_test() {
    debug!("setup test");
    Command::new("virsh")
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

    #[test]
    fn test_01() {
        run_test(|| info!("Running the test"))
    }
}
