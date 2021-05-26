mod common;
use common::config::VM_VCPU_COUNT;
use common::context::run_test;
use serial_test::serial;

mod init {
    use super::*;

    #[test]
    #[serial]
    fn test_init_driver() {
        run_test(|_drv| {
            // nothing to do
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
}

mod get {
    use super::*;

    #[test]
    #[serial]
    fn test_get_vcpu_count() {
        run_test(|drv| {
            assert_eq!(VM_VCPU_COUNT, drv.get_vcpu_count().unwrap());
        })
    }
}

mod pause {
    use super::*;

    #[test]
    #[serial]
    fn test_pause() {
        run_test(|mut drv| {
            drv.pause().unwrap();
        })
    }

    #[test]
    #[serial]
    fn test_double_pause() {
        run_test(|mut drv| {
            drv.pause().unwrap();
            drv.pause().unwrap();
        })
    }

    #[test]
    #[serial]
    fn test_double_resume() {
        run_test(|mut drv| {
            drv.resume().unwrap();
            drv.resume().unwrap();
        })
    }

    #[test]
    #[serial]
    fn test_pause_resume() {
        run_test(|mut drv| {
            drv.pause().unwrap();
            drv.resume().unwrap();
        })
    }

    #[test]
    #[serial]
    fn test_multiple_pause_resume() {
        run_test(|mut drv| {
            for _ in 0..50 {
                drv.pause().unwrap();
                drv.resume().unwrap();
            }
        })
    }
}

mod events {
    use super::*;
    use microvmi::api::{CrType, EventReplyType, EventType, InterceptType};

    #[test]
    #[serial]
    fn test_intercept_cr3_one() {
        run_test(|mut drv| {
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
        run_test(|mut drv| {
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
