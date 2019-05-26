pub enum DriverType {
    Dummy,
    Xen,
}

pub trait Introspectable {
    // connets to the hypervisor and initialize the VMI subsystem for a given domain
    fn new(&self);
    // destroys the VMI subsystem instance
    fn close(&self);
}
