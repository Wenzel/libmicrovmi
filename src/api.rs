pub enum DriverType {
    Dummy,
    Xen,
}

pub trait Introspectable {
    // read physical memory
    fn read_physical(&self, paddr: u64, count: u32) -> Result<Vec<u8>,&str>;

    // pause the VM
    fn pause(&self);

    // resume the VM
    fn resume(&self);
}
