use std::mem;
use std::convert::TryInto;
use std::iter::Iterator;
use std::vec::Vec;
use std::slice;

use crate::api;
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, TH32CS_SNAPPROCESS, Process32FirstW, Process32NextW, PROCESSENTRY32W};
use winapi::um::winnt::{HANDLE, PVOID};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::shared::minwindef::{TRUE, FALSE, BOOL, DWORD, ULONG};
use winapi::shared::ntstatus::{STATUS_INFO_LENGTH_MISMATCH, STATUS_SUCCESS};
use widestring::U16CString;
use ntapi::ntexapi::{NtQuerySystemInformation, SYSTEM_HANDLE_INFORMATION, SystemHandleInformation, SYSTEM_HANDLE_TABLE_ENTRY_INFO};

// iterator over processes
struct ProcessList {
    snapshot_handle: HANDLE,
    // whether we retrieved the first process already
    first_process_done: bool,
    proc_entry: PROCESSENTRY32W,
}

impl ProcessList {
    fn new() -> Self {
        let snapshot_handle = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
        };
        debug!("CreateToolHelp32Snapshot handle: {:?}", snapshot_handle);
        if snapshot_handle == INVALID_HANDLE_VALUE {
            panic!("CreateToolHelp32Snapshot failed !");
        }
        let mut list = ProcessList {
            snapshot_handle,
            first_process_done: false,
            proc_entry: unsafe {
                mem::MaybeUninit::<PROCESSENTRY32W>::zeroed().assume_init()
            },
        };
        // must set dwSize
        list.proc_entry.dwSize = mem::size_of::<PROCESSENTRY32W>().try_into().unwrap();
        list
    }
}

impl Iterator for ProcessList {
    type Item = PROCESSENTRY32W;

    fn next(&mut self) -> Option<Self::Item> {
        let res: BOOL = match self.first_process_done {
            false => {
                match unsafe { Process32FirstW(self.snapshot_handle, &mut self.proc_entry) } {
                    TRUE => {
                        self.first_process_done = true;
                        TRUE
                    },
                    _ => panic!("Process32FirstW failed !")
                }
            },
            true => {
                unsafe { Process32NextW(self.snapshot_handle, &mut self.proc_entry) }
            }
        };
        match res {
            FALSE => None,
            TRUE => Some(self.proc_entry),
            _ => panic!("Unexpected return value from Process32NextW"),
        }
    }
}

// unit struct
#[derive(Debug)]
pub struct HyperV;

impl HyperV {

    pub fn new(domain_name: &str) -> Self {
        debug!("HyperV driver init on {}", domain_name);
        let vmwp_name: U16CString = U16CString::from_str("vmwp.exe").unwrap();

        let process_list = ProcessList::new();
        for proc_entry in process_list {
            let exe_file = unsafe { U16CString::from_ptr_str(&proc_entry.szExeFile as *const u16) };
            let pid: DWORD = proc_entry.th32ProcessID;
            if exe_file == vmwp_name {
                debug!("Found Hyper-V VM process - PID: {}", pid);
                // find handles
                let mut size: ULONG = 1000;
                let mut status = STATUS_INFO_LENGTH_MISMATCH;
                let mut array_handle: Vec<SYSTEM_HANDLE_INFORMATION> = Vec::with_capacity(size as usize);
                let mut ret_len: ULONG = 0;
                while status == STATUS_INFO_LENGTH_MISMATCH {
                    array_handle = Vec::with_capacity(size as usize);
                    let ptr: PVOID = array_handle.as_mut_ptr() as PVOID;
                    status = unsafe {
                        NtQuerySystemInformation(SystemHandleInformation, ptr, size, &mut ret_len)
                    };
                    // double size
                    size = size * 2;
                }

                if status != STATUS_SUCCESS {
                    panic!("NtQuerySystemInformation failed !");
                }
                // set vector's new len
                unsafe { array_handle.set_len(ret_len.try_into().unwrap()) };

                debug!("NtQuerySystemInformation: Handle count: {}", array_handle[0].NumberOfHandles);
                let max_handles = array_handle[0].NumberOfHandles as usize;
                let dyn_array: &[SYSTEM_HANDLE_TABLE_ENTRY_INFO] = unsafe { slice::from_raw_parts(array_handle[0].Handles.as_ptr(), max_handles) };
                for index in 0..max_handles {
                    if dyn_array[index].UniqueProcessId == pid.try_into().unwrap() {
                        let handle = dyn_array[index].HandleValue;
                        debug!("[{}] vmwp.exe handle: {}", pid, handle);
                    }
                }
            }
        }

        let hyperv = HyperV;
        hyperv
    }

    fn close(&mut self) {
        debug!("HyperV driver close");
    }
}

impl api::Introspectable for HyperV {

}

impl Drop for HyperV {
    fn drop(&mut self) {
        self.close();
    }
}
