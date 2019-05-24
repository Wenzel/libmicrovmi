pub trait Introspectable {
    fn init(&self);
    fn close(&self);
}
