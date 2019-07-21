use crate::api;
use kvmi::{KVMi};
use std::io::Error;

// unit struct
#[derive(Debug)]
pub struct Kvm {
    kvmi: KVMi,
    expect_pause_ev: u32,
}

impl Kvm {

    pub fn new(domain_name: &str) -> Self {
        println!("KVM driver init on {}", domain_name);
        let socket_path = "/tmp/introspector";
        let kvm = Kvm {
            kvmi: KVMi::new(socket_path),
            expect_pause_ev: 0,
        };
        kvm
    }

    fn close(&mut self) {
        println!("KVM driver close");
    }
}

impl api::Introspectable for Kvm {

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),&str> {
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64,&str> {
        Ok(0)
    }

    fn pause(&mut self) {
        println!("KVM driver pause");
        self.expect_pause_ev = self.kvmi.pause()
            .expect("Failed to pause KVM VCPUs");
        println!("expected pause events: {}", self.expect_pause_ev);
    }

    fn resume(&self) {
        println!("KVM driver resume");
    }

}

impl Drop for Kvm {
    fn drop(&mut self) {
        self.close();
    }
}

