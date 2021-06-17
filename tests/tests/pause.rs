use super::IntegrationTest;
use microvmi::api::Introspectable;

fn pause(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
}

inventory::submit!(IntegrationTest {
    name: "pause",
    test_fn: pause
});

fn double_pause(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    drv.pause().unwrap();
}

inventory::submit!(IntegrationTest {
    name: "double_pause",
    test_fn: double_pause
});

fn double_resume(mut drv: Box<dyn Introspectable>) {
    drv.resume().unwrap();
    drv.resume().unwrap();
}

inventory::submit!(IntegrationTest {
    name: "double_resume",
    test_fn: double_resume
});

fn pause_resume(mut drv: Box<dyn Introspectable>) {
    drv.pause().unwrap();
    drv.resume().unwrap();
}

inventory::submit!(IntegrationTest {
    name: "pause_resume",
    test_fn: pause_resume
});

fn multiple_pause_resume(mut drv: Box<dyn Introspectable>) {
    for _ in 0..50 {
        drv.pause().unwrap();
        drv.resume().unwrap();
    }
}

inventory::submit!(IntegrationTest {
    name: "multiple_pause_resume",
    test_fn: multiple_pause_resume
});
