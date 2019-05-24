mod intro;
mod driver;
use crate::intro::Introspectable;

pub fn vmi_init() {
    println!("vmi init");

    let drv = driver::dummy::Dummy;
    drv.init();
}

pub fn vmi_close() {
    println!("vmi close");

    let drv = driver::dummy::Dummy;
    drv.close();
}
