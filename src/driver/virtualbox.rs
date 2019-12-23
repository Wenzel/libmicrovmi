use std::error::Error;

use crate::api::Introspectable;


// unit struct
#[derive(Debug)]
pub struct VBox;

impl VBox {
    pub fn new(domain_name: &str) -> Self {
        VBox
    }
}

impl Introspectable for VBox {

}

