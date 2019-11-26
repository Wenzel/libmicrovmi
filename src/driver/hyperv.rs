use std::convert::TryInto;
use std::iter::{Iterator, once};
use std::mem;
use std::slice;
use std::vec::Vec;
use std::ptr::{null, null_mut};
use std::ffi::{CString, OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::io::Error;

use crate::api;

use widestring::U16CString;
use ntapi::ntexapi::{
    NtQuerySystemInformation, SystemHandleInformation, SYSTEM_HANDLE_INFORMATION,
    SYSTEM_HANDLE_TABLE_ENTRY_INFO,
};
use ntapi::ntobapi::{
    NtDuplicateObject, NtQueryObject, ObjectTypeInformation,
    ObjectNameInformation, OBJECT_INFORMATION_CLASS, OBJECT_TYPE_INFORMATION,
    OBJECT_NAME_INFORMATION};
use winapi::shared::minwindef::{BOOL, DWORD, FALSE, TRUE, ULONG, USHORT};
use winapi::shared::ntdef::{NULL, LUID};
use winapi::shared::ntstatus::{STATUS_INFO_LENGTH_MISMATCH, STATUS_SUCCESS};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::tlhelp32::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use winapi::um::winnt::{
    HANDLE, PVOID, PROCESS_DUP_HANDLE, TOKEN_ADJUST_PRIVILEGES,
    SE_DEBUG_NAME, TOKEN_PRIVILEGES, SE_PRIVILEGE_ENABLED, DUPLICATE_SAME_ACCESS,
    GENERIC_READ, GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_ATTRIBUTE_NORMAL
};
use winapi::um::winbase::LookupPrivilegeValueA;
use winapi::um::processthreadsapi::{OpenProcess, OpenProcessToken, GetCurrentProcess};
use winapi::um::securitybaseapi::AdjustTokenPrivileges;
use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::ioapiset::{DeviceIoControl};
use winapi::um::winioctl::{CTL_CODE, FILE_DEVICE_UNKNOWN, METHOD_BUFFERED, FILE_ANY_ACCESS};


// iterator over processes
struct ProcessList {
    snapshot_handle: HANDLE,
    // whether we retrieved the first process already
    first_process_done: bool,
    proc_entry: PROCESSENTRY32W,
}

impl ProcessList {
    fn new() -> Self {
        let snapshot_handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        debug!("CreateToolHelp32Snapshot handle: {:?}", snapshot_handle);
        if snapshot_handle == INVALID_HANDLE_VALUE {
            panic!("CreateToolHelp32Snapshot failed !");
        }
        let mut list = ProcessList {
            snapshot_handle,
            first_process_done: false,
            proc_entry: unsafe { mem::MaybeUninit::<PROCESSENTRY32W>::zeroed().assume_init() },
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
            false => match unsafe { Process32FirstW(self.snapshot_handle, &mut self.proc_entry) } {
                TRUE => {
                    self.first_process_done = true;
                    TRUE
                }
                _ => panic!("Process32FirstW failed !"),
            },
            true => unsafe { Process32NextW(self.snapshot_handle, &mut self.proc_entry) },
        };
        match res {
            FALSE => None,
            TRUE => Some(self.proc_entry),
            _ => panic!("Unexpected return value from Process32NextW"),
        }
    }
}

struct VMWPHandleIter {
    pid: DWORD,
    vmwp_handles: Vec<SYSTEM_HANDLE_TABLE_ENTRY_INFO>,
    index: usize,
    max_handles: usize,
}

impl VMWPHandleIter {
    fn new(pid: DWORD) -> Self {
        // call NtQuerySystemInformation as much time as needed
        // with a growing buffer to get all handles
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

        debug!(
            "NtQuerySystemInformation: Handle count: {}",
            array_handle[0].NumberOfHandles
        );
        // need a usize to be used for array index
        let max_handles = array_handle[0].NumberOfHandles as usize;
        // build a new array from Handles, because Handles is of fixed size 1
        let real_array_handles: &[SYSTEM_HANDLE_TABLE_ENTRY_INFO] =
            unsafe { slice::from_raw_parts(array_handle[0].Handles.as_ptr(), max_handles) };

        let mut vmwp_handles = Vec::with_capacity(max_handles);
        for index in 0..max_handles {
            vmwp_handles.push(real_array_handles[index]);
        }

        VMWPHandleIter {
            pid,
            vmwp_handles,
            index: 0,
            max_handles,
        }
    }
}

impl Iterator for VMWPHandleIter {
    type Item = USHORT;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.max_handles {
            if self.vmwp_handles[self.index].UniqueProcessId == self.pid.try_into().unwrap() {
                let handle = self.vmwp_handles[self.index].HandleValue;
                self.index = self.index + 1;
                return Some(handle);
            } else {
                self.index = self.index + 1;
            }
        }
        None
    }
}

// unit struct
#[derive(Debug)]
pub struct HyperV;

impl HyperV {
    pub fn new(domain_name: &str) -> Self {
        debug!("HyperV driver init on {}", domain_name);

        Self::enable_se_debug_privilege();

        let vmwp_name: U16CString = U16CString::from_str("vmwp.exe").unwrap();

        let process_list = ProcessList::new();
        for proc_entry in process_list {
            let exe_file = unsafe { U16CString::from_ptr_str(&proc_entry.szExeFile as *const u16) };
            let pid: DWORD = proc_entry.th32ProcessID;
            if exe_file == vmwp_name {
                debug!("Found Hyper-V VM process - PID: {}", pid);
                // find handles
                let handle_list = VMWPHandleIter::new(pid);
                // get handle to vmwp process
                let worker_handle = unsafe {
                    OpenProcess(PROCESS_DUP_HANDLE, FALSE, pid)
                };
                if worker_handle == INVALID_HANDLE_VALUE {
                    panic!("Failed to open process");
                }
                debug!("vmwp.exe process handle: {:?}", worker_handle);
                for handle in handle_list {
                    // handle is a USHORT, cast it to HANDLE
                    let mut cur_handle: HANDLE = handle as HANDLE;
                    debug!("[{}] vmwp.exe handle: {:p}", pid, cur_handle);
                    // duplicate current handle into our process
                    let cur_proc_handle: HANDLE = unsafe { GetCurrentProcess() };
                    let mut duplicated_handle: HANDLE = INVALID_HANDLE_VALUE;

                    let res = unsafe {
                        NtDuplicateObject(worker_handle, handle as HANDLE, cur_proc_handle, &mut duplicated_handle, 0, FALSE.try_into().unwrap(), DUPLICATE_SAME_ACCESS)
                    };
                    if res != STATUS_SUCCESS {
                        continue;
                    }
                    // NtQueryObject type
                    let mut obj_type: OBJECT_TYPE_INFORMATION = unsafe { mem::MaybeUninit::<OBJECT_TYPE_INFORMATION>::zeroed().assume_init() };
                    let ptr_obj_type = &mut obj_type as *mut _ as PVOID;
                    Self::query_object(duplicated_handle, ObjectTypeInformation, ptr_obj_type);
                    
                    // check that type is "File"
                    let obj_buffer = unsafe { U16CString::from_ptr_str(obj_type.TypeName.Buffer) };
                    let file_cmp = U16CString::from_str("File").unwrap();
                    if obj_buffer == file_cmp {
                        debug!("type: File");
                        // NtQueryObject name
                        let mut obj_name: OBJECT_NAME_INFORMATION = unsafe { mem::MaybeUninit::<OBJECT_NAME_INFORMATION>::zeroed().assume_init() };
                        let ptr_obj_name = &mut obj_name as *mut _ as PVOID;
                        Self::query_object(duplicated_handle, ObjectNameInformation, ptr_obj_name);

                        // check that name is "\\Device\\00000"
                        let name_buffer = unsafe { U16CString::from_ptr_str(obj_name.Name.Buffer) }.to_string_lossy();
                        debug!("name: {}", name_buffer);
                        if name_buffer.starts_with("\\Device\\000000") {
                            debug!("Potential PT_HANDLE !");
                            // CreateFileW
                            let device_path: Vec<u16> = OsStr::new("\\\\.\\hvlckd").encode_wide().chain(once(0)).collect()    ;
                            let h_device = unsafe {
                                CreateFileW(device_path.as_ptr(),
                                            GENERIC_READ | GENERIC_WRITE,
                                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                                            null_mut(),
                                            OPEN_EXISTING,
                                            FILE_ATTRIBUTE_NORMAL,
                                            NULL)
                            };

                            if h_device == null_mut() {
                                panic!("CreateFileW failed !");
                            }
                            debug!("hdevice: {:?}", h_device);

                            let mut vec_buff: Vec<u8> = Vec::with_capacity(0x200);
                            let buff = vec_buff.as_mut_ptr() as *mut c_void;
                            let ioctl_friendly_name = CTL_CODE(FILE_DEVICE_UNKNOWN, 0x820, METHOD_BUFFERED, FILE_ANY_ACCESS);
                            let mut bytes_ret: DWORD = 0;
                            let part_ptr_handle = &mut cur_handle as *mut _ as *mut c_void;
                            let res = unsafe {
                                DeviceIoControl(h_device, ioctl_friendly_name, part_ptr_handle, mem::size_of::<HANDLE>().try_into().unwrap(),
                                                buff, 0x200, &mut bytes_ret, null_mut())
                            };

                            if res == FALSE {
                                panic!("DeviceIoControl failed: {}", Error::last_os_error());
                            }

                            if bytes_ret > 0 {
                                debug!("bytes !!");
                            }
                        }
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

    fn query_object(handle: HANDLE, query_type: OBJECT_INFORMATION_CLASS, ptr: PVOID) {
        let mut bytes_ret: u32 = 0;
        let mut status = unsafe {
            NtQueryObject(handle, query_type, NULL, 0, &mut bytes_ret)
        };
        if status != STATUS_INFO_LENGTH_MISMATCH {
            debug!("NtQueryObject failed");
        }

        status = unsafe {
            NtQueryObject(handle, query_type, ptr, bytes_ret, &mut bytes_ret)
        };
        if status != STATUS_SUCCESS {
            debug!("NtQueryObject failed");
        }
    }

    fn enable_se_debug_privilege() {
        // OpenProcessToken
        let cur_proc_handle = unsafe { GetCurrentProcess() };
        let mut h_token: HANDLE = null_mut();
        let mut res = unsafe {
            OpenProcessToken(cur_proc_handle, TOKEN_ADJUST_PRIVILEGES, &mut h_token)
        };
        if res == FALSE {
            panic!("Failed to get current process privilege token")
        }
        debug!("OpenProcessToken: OK");
        // LookupPrivilegeValue
        let mut luid_debug: LUID = unsafe { mem::MaybeUninit::<LUID>::zeroed().assume_init() };
        let se_debug_name = CString::new(SE_DEBUG_NAME).unwrap();
        res = unsafe {
            LookupPrivilegeValueA(null(), se_debug_name.as_ptr(), &mut luid_debug)
        };
        if res == FALSE {
            panic!("Failed to lookup current SE_DEBUG privileges");
        }
        debug!("LookupPrivilegeValue: OK");
        // AdjustPrivilege
        let mut token_priv: TOKEN_PRIVILEGES = unsafe { mem::MaybeUninit::<TOKEN_PRIVILEGES>::zeroed().assume_init() };
        token_priv.PrivilegeCount = 1;
        token_priv.Privileges[0].Luid = luid_debug;
        token_priv.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;
        res = unsafe {
            AdjustTokenPrivileges(h_token, FALSE, &mut token_priv, 0, null_mut(), null_mut())
        };
        if res == FALSE {
            panic!("Failed to elevate privileges to SeDebug");
        }
        debug!("AdjustTokenPrivileges: OK");
    }
}

impl api::Introspectable for HyperV {}

impl Drop for HyperV {
    fn drop(&mut self) {
        self.close();
    }
}
