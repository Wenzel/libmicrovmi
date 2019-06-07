pub enum DriverType {
    Dummy,
    Xen,
}

pub trait Introspectable {
    // pause the VM
    fn pause(&self);

    // resume the VM
    fn resume(&self);

    // destroy the VMI subsystem instance
    fn close(&mut self);
}
