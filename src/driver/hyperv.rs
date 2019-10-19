use std::error::Error;
use std::mem;
use std::convert::TryInto;

use crate::api;
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, TH32CS_SNAPPROCESS, Process32FirstW, Process32NextW, PROCESSENTRY32W};
use winapi::um::winnt::{HANDLE, WCHAR};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::shared::minwindef::{TRUE, FALSE};

// unit struct
#[derive(Debug)]
pub struct HyperV {
	a: i32
}

impl HyperV {

    pub fn new(domain_name: &str) -> Self {
        println!("HyperV driver init on {}", domain_name);
        // snapshot
        let snapshot_handle: HANDLE = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
        };
        println!("snapshot handle: {:?}", snapshot_handle);
        if snapshot_handle == INVALID_HANDLE_VALUE {
            panic!("CreateToolHelp32Snapshot failed !");
        }

        // set size of proc_entry before getting first process from snapshot
        let mut proc_entry: PROCESSENTRY32W = unsafe {
            mem::MaybeUninit::<PROCESSENTRY32W>::zeroed().assume_init()
        };
        proc_entry.dwSize = mem::size_of::<PROCESSENTRY32W>().try_into().unwrap();
        
        // get first process
        let mut res = unsafe {
            Process32FirstW(snapshot_handle, &mut proc_entry)
        };
        if res != TRUE {
            panic!("Process32FirstW failed !");
        }

        //let exe_file = U16String::from_ptr(proc_entry.szExeFile, 260);
        let mut exe_file = String::from_utf16_lossy(&proc_entry.szExeFile);
        println!("process: {}", exe_file);

        while unsafe { Process32NextW(snapshot_handle, &mut proc_entry) } != FALSE
        {
            // find vmwp.exe
            exe_file = String::from_utf16_lossy(&proc_entry.szExeFile);
            println!("process: {}", exe_file);
            if (exe_file == "vmwp.exe") {
                println!("found a VM process !");
            }
        }

        let hyperv = HyperV {
			a: 0
        };
        hyperv
    }

    fn close(&mut self) {
        println!("HyperV driver close");
    }
}

impl api::Introspectable for HyperV {

    fn read_physical(&self, paddr: u64, buf: &mut [u8]) -> Result<(),Box<dyn Error>> {
        Ok(())
    }

    fn get_max_physical_addr(&self) -> Result<u64,Box<dyn Error>> {
        Ok(0)
    }

    fn pause(&mut self) -> Result<(),Box<dyn Error>> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(),Box<dyn Error>> {
        Ok(())
    }

}

impl Drop for HyperV {
    fn drop(&mut self) {
        self.close();
    }
}
