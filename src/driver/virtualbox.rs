use std::error::Error;

use fdp::FDP;

use crate::api::Introspectable;


// unit struct
#[derive(Debug)]
pub struct VBox {
    fdp: FDP,
}

impl VBox {
    pub fn new(domain_name: &str) -> Self {
        // init FDP
        let fdp = FDP::new(domain_name);
        VBox {
            fdp
        }
    }
}

impl Introspectable for VBox {

}

