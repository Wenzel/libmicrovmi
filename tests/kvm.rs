mod common;

#[cfg(feature = "kvm")]
mod tests {
    use super::common::config::{KVMI_SOCKET, VIRSH_URI, VM_NAME, VM_VCPU_COUNT};
    use super::common::run_test_generic;
    use log::debug;
    use serial_test::serial;
    use std::panic;
    use std::process::{Command, Stdio};

    use microvmi::api::{
        CrType, DriverInitParam, DriverType, EventReplyType, EventType, InterceptType,
        Introspectable,
    };
    use microvmi::init;

    /// restore VM state from internal QEMU snapshot
    pub fn setup_test() {
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
    pub fn teardown_test() {
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

    // define run_test with setup / teardown
    pub fn run_test<T>(test: T) -> ()
    where
        T: Send + 'static,
        T: FnOnce() -> (),
    {
        run_test_generic(setup_test, teardown_test, test)
    }

    fn init_driver() -> Box<dyn Introspectable> {
        init(
            VM_NAME,
            Some(DriverType::KVM),
            Some(DriverInitParam::KVMiSocket(String::from(KVMI_SOCKET))),
        )
        .expect("Failed to init libmicrovmi")
    }

    #[test]
    #[serial]
    fn test_init_driver() {
        run_test(|| {
            init_driver();
        })
    }

    // TODO: this test timeout, and makes intercept_cr3 tests timeout as well
    // #[test]
    // #[serial]
    // fn test_init_driver_twice() {
    //     run_test(|| {
    //         let drv = init_driver();
    //         mem::drop(drv);
    //         let _drv = init_driver();
    //     })
    // }

    #[test]
    #[serial]
    fn test_get_vcpu_count() {
        run_test(|| {
            assert_eq!(VM_VCPU_COUNT, init_driver().get_vcpu_count().unwrap());
        })
    }

    #[test]
    #[serial]
    fn test_pause() {
        run_test(|| {
            let mut drv = init_driver();
            drv.pause().unwrap();
        })
    }

    #[test]
    #[serial]
    fn test_double_pause() {
        run_test(|| {
            let mut drv = init_driver();
            drv.pause().unwrap();
            drv.pause().unwrap();
        })
    }

    #[test]
    #[serial]
    fn test_double_resume() {
        run_test(|| {
            let mut drv = init_driver();
            drv.resume().unwrap();
            drv.resume().unwrap();
        })
    }

    #[test]
    #[serial]
    fn test_pause_resume() {
        run_test(|| {
            let mut drv = init_driver();
            drv.pause().unwrap();
            drv.resume().unwrap();
        })
    }

    #[test]
    #[serial]
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
    #[serial]
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
    #[serial]
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

    // get_driver_type
    #[test]
    #[serial]
    fn test_get_driver_type() {
        run_test(|| {
            assert_eq!(DriverType::KVM, init_driver().get_driver_type());
        })
    }
}
