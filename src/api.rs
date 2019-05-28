pub enum DriverType {
    Dummy,
    Xen,
}

pub trait Introspectable {
    // destroys the VMI subsystem instance
    fn close(&self);
}
