use crate::api;
use kvmi::{KVMi};

// unit struct
#[derive(Debug)]
pub struct Kvm {
    kvmi: KVMi,
}

impl Kvm {

    pub fn new(domain_name: &str) -> Self {
        println!("KVM driver init on {}", domain_name);
        let socket_path = "/tmp/introspector";
        let kvm = Kvm {
            kvmi: KVMi::new(socket_path),
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

    fn pause(&self) {
        println!("KVM driver pause");
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

