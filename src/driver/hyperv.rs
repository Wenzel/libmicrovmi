use std::mem;
use std::convert::TryInto;
use std::iter::Iterator;

use crate::api;
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, TH32CS_SNAPPROCESS, Process32FirstW, Process32NextW, PROCESSENTRY32W};
use winapi::um::winnt::{HANDLE, PVOID};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::shared::minwindef::{TRUE, FALSE, BOOL, DWORD, ULONG};
use widestring::U16CString;
use ntapi::ntexapi::{NtQuerySystemInformation, SYSTEM_HANDLE_INFORMATION};

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
            }
        }

        let mut arr_handle: [SYSTEM_HANDLE_INFORMATION; 1000] = [unsafe { mem::MaybeUninit::<SYSTEM_HANDLE_INFORMATION>::zeroed().assume_init() }; 1000];
        let ptr: PVOID = arr_handle.as_mut_ptr() as PVOID;
        let size: ULONG = 1000;
        let mut ret_len: ULONG = 0;
        // TODO: use SYSTEM_INFORMATION_CLASS::SystemHandleInformation Enum ? howto ?
        let status = unsafe {
            NtQuerySystemInformation(16, ptr, size, &mut ret_len)
        };

        debug!("NtQueryInformation: {}, len: {}", status, ret_len);

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
