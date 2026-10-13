use microvmi::api::events::Event;
use microvmi::api::Introspectable;

pub mod events;
pub mod get;
pub mod init;
pub mod pause;
// same condition as init_context picking the VirtualBox context
#[cfg(all(
    any(feature = "virtualbox-api", feature = "virtualbox-vmi"),
    not(feature = "kvm")
))]
pub mod virtualbox;

#[derive(Debug)]
pub struct IntegrationTest {
    pub name: &'static str,
    pub test_fn: fn(Box<dyn Introspectable>),
}

inventory::collect!(IntegrationTest);

/// The next event, running `activity` (something making the guest write the intercepted
/// register) each second there is none.
fn next_event(drv: &mut dyn Introspectable, activity: &mut dyn FnMut()) -> Event {
    for _ in 0..10 {
        if let Some(event) = drv.listen(1000).unwrap() {
            return event;
        }
        activity();
    }
    panic!("no event in 10 s of guest activity");
}
