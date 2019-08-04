use std::error::Error;
use crate::api;
use kvmi::{KVMi, KVMiEventType};

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

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),Box<Error>> {
        Ok(self.kvmi.read_physical(paddr, buf)?)
    }

    fn get_max_physical_addr(&self) -> Result<u64,Box<Error>> {
        // No API in KVMi at the moment
        // fake 512MB
        let max_addr = 1024 * 1024 * 512;
        Ok(max_addr)
    }

    fn pause(&mut self) -> Result<(),Box<Error>> {
        println!("KVM driver pause");
        // already paused ?
        if self.expect_pause_ev > 0 {
            ()
        }

        self.expect_pause_ev = self.kvmi.pause()?;
        println!("expected pause events: {}", self.expect_pause_ev);
        Ok(())
    }

    fn resume(&mut self) -> Result<(),Box<Error>> {
        println!("KVM driver resume");
        // already resumed ?
        if self.expect_pause_ev == 0{
            ()
        }

        while self.expect_pause_ev > 0 {
            // wait
            self.kvmi.wait_event(1000)?;
            // pop
            let kvmi_event = self.kvmi.pop_event()?;
            match kvmi_event.kind {
                KVMiEventType::PauseVCPU => {
                    println!("Received Pause Event");
                    self.expect_pause_ev -= 1;
                    self.kvmi.reply_continue(&kvmi_event)?;
                }
                _ => panic!("Unexpected {:?} event type while resuming VM", kvmi_event.kind),
            }
        }
        Ok(())
    }

}

impl Drop for Kvm {
    fn drop(&mut self) {
        self.close();
    }
}

