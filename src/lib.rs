pub mod api;
mod driver;

use api::Introspectable;

pub fn vmi_init() -> impl Introspectable {
    println!("vmi init");

    let drv = driver::dummy::Dummy;
    drv.new();
    drv
}
