pub mod consts;
pub mod error;
mod libxenctrl;

#[macro_use]
mod macros;

use log::debug;

use self::consts::PAGE_SIZE;
use libxenctrl::LibXenCtrl;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::{
    alloc::{alloc_zeroed, Layout},
    convert::{From, TryFrom, TryInto},
    ffi::{self, c_void},
    io::Error,
    mem,
    os::raw::{c_int, c_uint},
    ptr::{null_mut, NonNull},
    slice,
};

pub use xenctrl_sys::xenmem_access_t;
use xenctrl_sys::{
    xc_cx_stat, xc_error_code, xc_interface, xc_px_stat, xc_px_val, xentoollog_logger,
};
use xenvmevent_sys::{
    vm_event_back_ring, vm_event_request_t, vm_event_response_t, vm_event_sring,
    VM_EVENT_REASON_MEM_ACCESS, VM_EVENT_REASON_MOV_TO_MSR, VM_EVENT_REASON_SINGLESTEP,
    VM_EVENT_REASON_SOFTWARE_BREAKPOINT, VM_EVENT_REASON_WRITE_CTRLREG, VM_EVENT_X86_CR0,
    VM_EVENT_X86_CR3, VM_EVENT_X86_CR4,
};

// re-exported definitions
pub use xenctrl_sys::{
    hvm_hw_cpu, hvm_save_descriptor, xc_cpuinfo_t, xc_domaininfo_t, xc_physinfo_t, xc_vcpuinfo_t,
    XEN_DOMCTL_DEBUG_OP_SINGLE_STEP_OFF, XEN_DOMCTL_DEBUG_OP_SINGLE_STEP_ON, __HVM_SAVE_TYPE_CPU,
};

use error::XcError;

/// Converts a `u32` to `xenmem_access_t`, avoiding unsafe transmute.
fn access_from_u32(value: u32) -> Result<xenmem_access_t, XcError> {
    match value {
        0 => Ok(xenmem_access_t::XENMEM_access_n),
        1 => Ok(xenmem_access_t::XENMEM_access_r),
        2 => Ok(xenmem_access_t::XENMEM_access_w),
        3 => Ok(xenmem_access_t::XENMEM_access_rw),
        4 => Ok(xenmem_access_t::XENMEM_access_x),
        5 => Ok(xenmem_access_t::XENMEM_access_rx),
        6 => Ok(xenmem_access_t::XENMEM_access_wx),
        7 => Ok(xenmem_access_t::XENMEM_access_rwx),
        8 => Ok(xenmem_access_t::XENMEM_access_rx2rw),
        9 => Ok(xenmem_access_t::XENMEM_access_n2rwx),
        10 => Ok(xenmem_access_t::XENMEM_access_default),
        _ => Err(XcError::new("Invalid access value")),
    }
}

#[derive(TryFromPrimitive, IntoPrimitive, Debug, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum XenCr {
    Cr0 = VM_EVENT_X86_CR0,
    Cr3 = VM_EVENT_X86_CR3,
    Cr4 = VM_EVENT_X86_CR4,
}

#[derive(Debug, Copy, Clone)]
pub enum XenEventType {
    Cr {
        cr_type: XenCr,
        new: u64,
        old: u64,
    },
    Msr {
        msr_type: u32,
        value: u64,
    },
    Breakpoint {
        gfn: u64,
        gpa: u64,
        insn_len: u8,
    },
    Pagefault {
        gva: u64,
        gpa: u64,
        access: xenmem_access_t,
        view: u16,
    },
    Singlestep {
        gfn: u64,
    },
}

#[derive(Debug)]
pub struct XenControl {
    handle: NonNull<xc_interface>,
    libxenctrl: LibXenCtrl,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct XcVcpuInfo {
    pub vcpu: u32,
    pub online: u8,
    pub blocked: u8,
    pub running: u8,
    pub cpu_time: u64,
    pub cpu: u32,
}

impl From<xc_vcpuinfo_t> for XcVcpuInfo {
    fn from(value: xc_vcpuinfo_t) -> Self {
        Self {
            vcpu: value.vcpu,
            online: value.online,
            blocked: value.blocked,
            running: value.running,
            cpu_time: value.cpu_time,
            cpu: value.cpu,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PxStat {
    pub total: u8,
    pub usable: u8,
    pub last: u8,
    pub cur: u8,
    pub transition_table: Vec<u64>,
    pub values: Vec<xc_px_val>,
}

#[derive(Debug, Clone, Default)]
pub struct CxStat {
    pub nr: u32,
    pub last: u32,
    pub idle_time: u64,
    pub triggers: Vec<u64>,
    pub residencies: Vec<u64>,
    pub nr_pc: u32,
    pub nr_cc: u32,
    pub pc: Vec<u64>,
    pub cc: Vec<u64>,
}

impl XenControl {
    pub fn new(
        logger: Option<&mut xentoollog_logger>,
        dombuild_logger: Option<&mut xentoollog_logger>,
        open_flags: u32,
    ) -> Result<Self, XcError> {
        let libxenctrl = unsafe { LibXenCtrl::new()? };

        #[allow(clippy::redundant_closure)]
        let xc_handle = (libxenctrl.interface_open)(
            logger.map_or_else(|| null_mut(), |l| l as *mut _),
            dombuild_logger.map_or_else(|| null_mut(), |l| l as *mut _),
            open_flags,
        );

        NonNull::new(xc_handle)
            .ok_or_else(|| {
                let desc = (libxenctrl.error_code_to_desc)(xc_error_code::XC_INTERNAL_ERROR as _);
                XcError::new(unsafe { ffi::CStr::from_ptr(desc) }.to_str().unwrap())
            })
            .map(|handle| XenControl { handle, libxenctrl })
    }

    pub fn new_default() -> Result<Self, XcError> {
        Self::new(None, None, 0)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let mut xc = XenControl::new_default()?;
    /// let dom_info = xc.domain_getinfo(1)?;
    /// println!("dominfo: {:?}", dom_info);
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn domain_getinfo(&self, domid: u32) -> Result<Option<xc_domaininfo_t>, XcError> {
        let xc = self.handle.as_ptr();
        let mut domain_info: xc_domaininfo_t = unsafe { mem::zeroed() };
        (self.libxenctrl.clear_last_error)(xc);
        let count = (self.libxenctrl.domain_getinfolist)(xc, domid, 1, &mut domain_info);
        // xc_domain_getinfolist returns domains starting from first_domain,
        // so we need to verify the returned domain matches the requested one
        last_error!(
            self,
            if count == 1 && u32::from(domain_info.domain) == domid {
                Some(domain_info)
            } else {
                None
            },
            count
        )
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError, XEN_DOMCTL_DEBUG_OP_SINGLE_STEP_ON};
    ///
    /// # let mut xc = XenControl::new_default()?;
    /// xc.domain_debug_control(1, XEN_DOMCTL_DEBUG_OP_SINGLE_STEP_ON, 0)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn domain_debug_control(&self, domid: u32, op: u32, vcpu: u32) -> Result<(), XcError> {
        debug!("domain_debug_control: op: {}, vcpu: {}", op, vcpu);
        (self.libxenctrl.clear_last_error)(self.handle.as_ptr());
        let rc = (self.libxenctrl.domain_debug_control)(self.handle.as_ptr(), domid, op, vcpu);
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let mut xc = XenControl::new_default()?;
    /// let hvm_cpu = xc.domain_hvm_getcontext_partial(1, 0)?;
    /// println!("RIP: {:?}", hvm_cpu.rip);
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn domain_hvm_getcontext_partial(
        &self,
        domid: u32,
        vcpu: u16,
    ) -> Result<hvm_hw_cpu, XcError> {
        let xc = self.handle.as_ptr();
        let mut hvm_cpu: hvm_hw_cpu = unsafe { mem::zeroed() };
        // cast to mut c_void*
        let hvm_cpu_ptr = &mut hvm_cpu as *mut _ as *mut c_void;
        let hvm_size: u32 = mem::size_of::<hvm_hw_cpu>().try_into().unwrap();
        let hvm_save_cpu: __HVM_SAVE_TYPE_CPU = unsafe { mem::zeroed() };
        let hvm_save_code_cpu: u16 = mem::size_of_val(&hvm_save_cpu.c).try_into().unwrap();

        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.domain_hvm_getcontext_partial)(
            xc,
            domid,
            hvm_save_code_cpu,
            vcpu,
            hvm_cpu_ptr,
            hvm_size,
        );
        last_error!(self, hvm_cpu, rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    /// # use std::convert::TryInto;
    ///
    /// # let mut xc = XenControl::new_default()?;
    /// let (buffer, mut cpu, size) = xc.domain_hvm_getcontext(1, 0)?;
    /// // set RIP
    /// cpu.rip = 0xdeadbeef;
    /// xc.domain_hvm_setcontext(1, buffer, size.try_into().unwrap())?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn domain_hvm_setcontext(
        &self,
        domid: u32,
        buffer: *mut c_uint,
        size: usize,
    ) -> Result<(), XcError> {
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc =
            (self.libxenctrl.domain_hvm_setcontext)(xc, domid, buffer, size.try_into().unwrap());
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let mut xc = XenControl::new_default()?;
    /// let (buffer, mut cpu, size) = xc.domain_hvm_getcontext(1, 0)?;
    /// println!("RIP: {:?}", cpu.rip);
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn domain_hvm_getcontext(
        &self,
        domid: u32,
        vcpu: u16,
    ) -> Result<(*mut c_uint, hvm_hw_cpu, u32), XcError> {
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        // calling with no arguments --> return is the size of buffer required for storing the HVM context
        let size =
            (self.libxenctrl.domain_hvm_getcontext)(xc, domid, std::ptr::null_mut::<u32>(), 0);
        let layout =
            Layout::from_size_align(size.try_into().unwrap(), mem::align_of::<u8>()).unwrap();
        #[allow(clippy::cast_ptr_alignment)]
        let buffer = unsafe { alloc_zeroed(layout) as *mut c_uint };
        (self.libxenctrl.clear_last_error)(xc);
        // Locate runtime CPU registers in the context record. This function returns information about the context of a hvm domain.
        (self.libxenctrl.domain_hvm_getcontext)(xc, domid, buffer, size.try_into().unwrap());
        let mut offset: u32 = 0;
        let hvm_save_cpu: __HVM_SAVE_TYPE_CPU = unsafe { mem::zeroed() };
        let hvm_save_code_cpu: u16 = mem::size_of_val(&hvm_save_cpu.c).try_into().unwrap();
        let mut cpu_ptr: *mut hvm_hw_cpu = std::ptr::null_mut();
        unsafe {
            // The execution context of the hvm domain is stored in the buffer struct we passed in domain_hvm_getcontext(). We iterate from the beginning address of this struct until we find the particular descriptor having typecode HVM_SAVE_CODE(CPU) which gives us the info about the registers in the particular vcpu.
            // Note that domain_hvm_getcontext_partial(), unlike domain_hvm_getcontext() returns only the descriptor struct having a particular typecode passed as one of its argument.
            while offset < size.try_into().unwrap() {
                let buffer_ptr = buffer as usize;
                let descriptor: *mut hvm_save_descriptor =
                    (buffer_ptr + offset as usize) as *mut hvm_save_descriptor;
                let diff: u32 = mem::size_of::<hvm_save_descriptor>().try_into().unwrap();
                offset += diff;
                if (*descriptor).typecode == hvm_save_code_cpu && (*descriptor).instance == vcpu {
                    cpu_ptr = (buffer_ptr + offset as usize) as *mut hvm_hw_cpu;
                    break;
                }

                offset += (*descriptor).length;
            }
        }
        last_error!(self, (buffer, *cpu_ptr, size.try_into().unwrap()))
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let mut xc = XenControl::new_default()?;
    /// let (_ring_page, back_ring, remote_port) = xc.monitor_enable(1)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn monitor_enable(
        &mut self,
        domid: u32,
    ) -> Result<(*mut vm_event_sring, vm_event_back_ring, u32), XcError> {
        debug!("monitor_enable");
        let xc = self.handle.as_ptr();
        let mut remote_port: u32 = 0;
        (self.libxenctrl.clear_last_error)(xc);
        let void_ring_page: *mut c_void =
            (self.libxenctrl.monitor_enable)(xc, domid.try_into().unwrap(), &mut remote_port);
        if void_ring_page.is_null() {
            return Err(XcError::new(
                "Failed to enable event monitor ring: ring page is null",
            ));
        }
        let ring_page = void_ring_page as *mut vm_event_sring;
        unsafe {
            (*ring_page).req_prod = 0;
            (*ring_page).rsp_prod = 0;
            (*ring_page).req_event = 1;
            (*ring_page).rsp_event = 1;
            (*ring_page).pvt.pvt_pad = mem::zeroed();
            (*ring_page).__pad = mem::zeroed();
        }
        // BACK_RING_INIT(&back_ring, ring_page, XC_PAGE_SIZE);
        let mut back_ring: vm_event_back_ring = unsafe { mem::zeroed() };
        back_ring.rsp_prod_pvt = 0;
        back_ring.req_cons = 0;
        back_ring.nr_ents = __RING_SIZE!(ring_page, PAGE_SIZE);
        back_ring.sring = ring_page;
        Ok((ring_page, back_ring, remote_port))
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    /// # use xenvmevent_sys::vm_event_back_ring;
    /// # use std::mem::MaybeUninit;
    ///
    /// # let xc = XenControl::new_default()?;
    /// // assume back_ring from `monitor_enable`
    /// let mut back_ring: vm_event_back_ring = Default::default();
    /// let req = xc.get_request(&mut back_ring)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn get_request(
        &self,
        back_ring: &mut vm_event_back_ring,
    ) -> Result<vm_event_request_t, XcError> {
        let mut req_cons = back_ring.req_cons;
        let req_from_ring = RING_GET_REQUEST!(back_ring, req_cons);
        req_cons += 1;
        back_ring.req_cons = req_cons;
        unsafe {
            (*(back_ring.sring)).req_event = 1 + req_cons;
        }
        Ok(req_from_ring)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    /// # use xenvmevent_sys::{vm_event_response_t, vm_event_back_ring};
    /// # use std::mem::MaybeUninit;
    ///
    /// # let xc = XenControl::new_default()?;
    /// let mut rsp = unsafe { MaybeUninit::<vm_event_response_t>::zeroed().assume_init() };
    /// // assume back_ring from `monitor_enable`
    /// let mut back_ring: vm_event_back_ring = Default::default();
    /// let event_type = xc.put_response(&mut rsp, &mut back_ring)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn put_response(
        &self,
        rsp: &mut vm_event_response_t,
        back_ring: &mut vm_event_back_ring,
    ) -> Result<(), XcError> {
        let mut rsp_prod = back_ring.rsp_prod_pvt;
        let rsp_dereferenced = *rsp;
        RING_PUT_RESPONSE!(back_ring, rsp_prod, rsp_dereferenced);
        rsp_prod += 1;
        back_ring.rsp_prod_pvt = rsp_prod;
        RING_PUSH_RESPONSES!(back_ring);
        Ok(())
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    /// # use xenvmevent_sys::vm_event_request_t;
    ///
    /// # let xc = XenControl::new_default()?;
    /// // assume req from `get_request`
    /// let req: vm_event_request_t = Default::default();
    /// let event_type = xc.get_event_type(req)?;
    /// println!("XenEventType: {:?}", event_type);
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn get_event_type(&self, req: vm_event_request_t) -> Result<XenEventType, XcError> {
        let ev_type: XenEventType;
        unsafe {
            ev_type = match req.reason {
                VM_EVENT_REASON_WRITE_CTRLREG => XenEventType::Cr {
                    cr_type: XenCr::try_from(req.u.write_ctrlreg.index).unwrap(),
                    new: req.u.write_ctrlreg.new_value,
                    old: req.u.write_ctrlreg.old_value,
                },
                VM_EVENT_REASON_MOV_TO_MSR => XenEventType::Msr {
                    msr_type: req.u.mov_to_msr.msr.try_into().unwrap(),
                    value: req.u.mov_to_msr.new_value,
                },
                VM_EVENT_REASON_SOFTWARE_BREAKPOINT => XenEventType::Breakpoint {
                    gfn: req.u.software_breakpoint.gfn,
                    gpa: 0, // not available
                    insn_len: req.u.software_breakpoint.insn_length.try_into().unwrap(),
                },
                VM_EVENT_REASON_MEM_ACCESS => XenEventType::Pagefault {
                    gva: req.u.mem_access.gla,
                    gpa: 0, // not available
                    access: access_from_u32(req.u.mem_access.flags)?,
                    view: 0,
                },
                VM_EVENT_REASON_SINGLESTEP => XenEventType::Singlestep {
                    gfn: req.u.singlestep.gfn,
                },
                _ => unimplemented!(),
            };
        }
        Ok(ev_type)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// xc.monitor_disable(1)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn monitor_disable(&self, domid: u32) -> Result<(), XcError> {
        debug!("monitor_disable");
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.monitor_disable)(xc, domid.try_into().unwrap());
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// xc.domain_pause(1)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn domain_pause(&self, domid: u32) -> Result<(), XcError> {
        debug!("domain pause");
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.domain_pause)(xc, domid);
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// xc.domain_unpause(1)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn domain_unpause(&self, domid: u32) -> Result<(), XcError> {
        debug!("domain_unpause");
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.domain_unpause)(xc, domid);
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// xc.monitor_software_breakpoint(1, true)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn monitor_software_breakpoint(&self, domid: u32, enable: bool) -> Result<(), XcError> {
        debug!("monitor_software_breakpoint: {}", enable);
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.monitor_software_breakpoint)(xc, domid, enable);
        if rc < 0 {
            debug!("The error is {}", Error::last_os_error());
        }
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let sysenter_cs_index = 0x174;
    /// xc.monitor_mov_to_msr(1, sysenter_cs_index, true)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn monitor_mov_to_msr(&self, domid: u32, msr: u32, enable: bool) -> Result<(), XcError> {
        debug!("monitor_mov_to_msr: {:x} {}", msr, enable);
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.monitor_mov_to_msr)(xc, domid.try_into().unwrap(), msr, enable);
        if rc < 0 {
            debug!("The error is {}", Error::last_os_error());
        }
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// xc.monitor_singlestep(1, true)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn monitor_singlestep(&self, domid: u32, enable: bool) -> Result<(), XcError> {
        debug!("monitor_singlestep: {}", enable);
        (self.libxenctrl.clear_last_error)(self.handle.as_ptr());
        let rc = (self.libxenctrl.monitor_singlestep)(
            self.handle.as_ptr(),
            domid.try_into().unwrap(),
            enable,
        );
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, XenCr, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// xc.monitor_write_ctrlreg(1, XenCr::Cr3, true, false, false)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn monitor_write_ctrlreg(
        &self,
        domid: u32,
        index: XenCr,
        enable: bool,
        sync: bool,
        onchangeonly: bool,
    ) -> Result<(), XcError> {
        debug!("monitor_write_ctrlreg: {:?} {}", index, enable);
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.monitor_write_ctrlreg)(
            xc,
            domid.try_into().unwrap(),
            index as u16,
            enable,
            sync,
            onchangeonly,
        );
        if rc < 0 {
            debug!("The error is {}", Error::last_os_error());
        }
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, xenmem_access_t, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// xc.set_mem_access(1, xenmem_access_t::XENMEM_access_x, 0x1234, 1)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn set_mem_access(
        &self,
        domid: u32,
        access: xenmem_access_t,
        first_pfn: u64,
        nr: u32,
    ) -> Result<(), XcError> {
        debug!("set_mem_access: {:?} on pfn {}", access, first_pfn);
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc =
            (self.libxenctrl.set_mem_access)(xc, domid.try_into().unwrap(), access, first_pfn, nr);
        last_error!(self, (), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let page_access = xc.get_mem_access(1, 0x1234)?;
    /// println!("XenPageAccess: {:?}", page_access);
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn get_mem_access(&self, domid: u32, pfn: u64) -> Result<xenmem_access_t, XcError> {
        debug!("get_mem_access");
        let xc = self.handle.as_ptr();
        let mut access: xenmem_access_t = xenmem_access_t::XENMEM_access_default;
        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.get_mem_access)(xc, domid.try_into().unwrap(), pfn, &mut access);
        last_error!(self, access, rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let max_gfn = xc.domain_maximum_gpfn(1)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn domain_maximum_gpfn(&self, domid: u32) -> Result<u64, XcError> {
        debug!("domain_maximum_gfn");
        let xc = self.handle.as_ptr();
        #[allow(unused_assignments)]
        (self.libxenctrl.clear_last_error)(xc);
        let mut max_gpfn: u64 = 0;
        let rc =
            (self.libxenctrl.domain_maximum_gpfn)(xc, domid.try_into().unwrap(), &mut max_gpfn);
        last_error!(self, max_gpfn, rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let vcpuinfo = xc.vcpu_getinfo(1, 1)?;
    /// println!("XcVcpuInfo: {:?}", vcpuinfo);
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn vcpu_getinfo(&self, domid: u32, vcpu: u32) -> Result<XcVcpuInfo, XcError> {
        debug!("vcpu_getinfo");
        let xc = self.handle.as_ptr();
        let mut vcpu_info = unsafe { mem::zeroed() };

        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.vcpu_getinfo)(xc, domid, vcpu, &mut vcpu_info);

        last_error!(self, XcVcpuInfo::from(vcpu_info), rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let physinfo = xc.physinfo()?;
    /// println!("threads_per_code: {}, cores_per_socket: {}, nr_cpus: {}", physinfo.threads_per_core, physinfo.cores_per_socket, physinfo.nr_cpus);
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn physinfo(&self) -> Result<xc_physinfo_t, XcError> {
        debug!("physinfo");
        let xc = self.handle.as_ptr();
        let mut physinfo = unsafe { mem::zeroed() };

        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.physinfo)(xc, &mut physinfo);

        last_error!(self, physinfo, rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let vec = xc.get_cpuinfo(8)?;
    /// for (i, vcpuinfo)  in vec.iter().enumerate() {
    ///     println!("[{}] idletime: {}", i, vcpuinfo.idletime);
    /// }
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn get_cpuinfo(&self, max_cpus: usize) -> Result<Vec<xc_cpuinfo_t>, XcError> {
        debug!("get_cpuinfo");
        let mut infos = vec![xc_cpuinfo_t { idletime: 0 }; max_cpus];
        let mut nr_cpus: i32 = 0;

        let rc = (self.libxenctrl.get_cpuinfo)(
            self.handle.as_ptr(),
            infos.len() as i32,
            infos.as_mut_ptr() as _,
            &mut nr_cpus,
        );

        infos.truncate(nr_cpus as usize);

        last_error!(self, infos, rc)
    }

    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let avg_freq = xc.get_cpufreq_avg(1)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn get_cpufreq_avg(&self, cpuid: u32) -> Result<u32, XcError> {
        debug!("get_cpufreq_avg");
        let xc = self.handle.as_ptr();
        let mut freq: c_int = 0;

        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.get_cpufreq_avgfreq)(xc, cpuid as c_int, &mut freq);

        last_error!(self, freq as _, rc)
    }

    /// As [PxStat] can hold quite large structures, you need to create an empty one using [Default] trait and
    /// provide it as `px_stat` to this function that will update the values.
    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, PxStat, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let mut pxstat = Default::default();
    /// xc.get_pxstat(1, &mut pxstat)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn get_pxstat(&self, cpuid: u32, px_stat: &mut PxStat) -> Result<(), XcError> {
        debug!("get_pxstat");
        let xc = self.handle.as_ptr();

        let mut max_px: c_int = 0;

        (self.libxenctrl.clear_last_error)(xc);
        let ret = (self.libxenctrl.get_max_px)(xc, cpuid as _, &mut max_px);

        if ret != 0 {
            return last_error!(self, ());
        }

        px_stat.values.resize(
            max_px as _,
            xc_px_val {
                freq: 0,
                residency: 0,
                count: 0,
            },
        );

        px_stat
            .transition_table
            .resize((max_px * max_px) as usize, 0);

        let mut px_stat_ffi = xc_px_stat {
            total: 0,
            usable: 0,
            last: 0,
            cur: 0,
            trans_pt: px_stat.transition_table.as_mut_ptr(),
            pt: px_stat.values.as_mut_ptr(),
        };

        (self.libxenctrl.clear_last_error)(xc);
        let ret = (self.libxenctrl.get_pxstat)(xc, cpuid as c_int, &mut px_stat_ffi);

        if ret == 0 {
            px_stat.total = px_stat_ffi.total;
            px_stat.usable = px_stat_ffi.usable;
            px_stat.last = px_stat_ffi.last;
            px_stat.cur = px_stat_ffi.cur;
        }

        last_error!(self, (), ret)
    }

    /// As [CxStat] can hold quite large structures, you need to create an empty one using [Default] trait and
    /// provide it as `cx_stat` to this function that will update the values.
    /// # Examples
    ///
    /// ```no_run
    /// # use xenctrl::{XenControl, CxStat, error::XcError};
    ///
    /// # let xc = XenControl::new_default()?;
    /// let mut cxstat = Default::default();
    /// xc.get_pxstat(1, &mut cxstat)?;
    /// # Ok::<(), XcError>(())
    /// ```
    pub fn get_cxstat(&self, cpuid: u32, cx_stat: &mut CxStat) -> Result<(), XcError> {
        debug!("get_cxstat");
        let xc = self.handle.as_ptr();
        let mut max_cx: c_int = 0;

        (self.libxenctrl.clear_last_error)(xc);
        let ret = (self.libxenctrl.get_max_cx)(xc, cpuid as _, &mut max_cx);

        if ret != 0 {
            return last_error!(self, ());
        }

        const MAX_PKG_RESIDENCIES: usize = 12;
        const MAX_CORE_RESIDENCIES: usize = 8;

        cx_stat.triggers.resize(max_cx as _, 0);
        cx_stat.residencies.resize(max_cx as _, 0);
        cx_stat.pc.resize(MAX_PKG_RESIDENCIES, 0);
        cx_stat.cc.resize(MAX_CORE_RESIDENCIES, 0);

        let mut cx_stat_ffi = xc_cx_stat {
            nr: max_cx as u32,
            last: 0,
            idle_time: 0,
            triggers: cx_stat.triggers.as_mut_ptr(),
            residencies: cx_stat.residencies.as_mut_ptr(),
            nr_pc: MAX_PKG_RESIDENCIES as u32,
            nr_cc: MAX_CORE_RESIDENCIES as u32,
            pc: cx_stat.pc.as_mut_ptr(),
            cc: cx_stat.cc.as_mut_ptr(),
        };

        (self.libxenctrl.clear_last_error)(xc);
        (self.libxenctrl.get_cxstat)(xc, cpuid as c_int, &mut cx_stat_ffi);

        if ret == 0 {
            cx_stat.nr = cx_stat_ffi.nr;
            cx_stat.last = cx_stat_ffi.last;
            cx_stat.idle_time = cx_stat_ffi.idle_time;
            cx_stat.nr_pc = cx_stat_ffi.nr_pc;
            cx_stat.nr_cc = cx_stat_ffi.nr_cc;
        }

        last_error!(self, (), ret)
    }

    fn close(&mut self) -> Result<(), XcError> {
        debug!("closing");
        let xc = self.handle.as_ptr();
        (self.libxenctrl.clear_last_error)(xc);
        let rc = (self.libxenctrl.interface_close)(xc);
        last_error!(self, (), rc)
    }
}

impl Drop for XenControl {
    fn drop(&mut self) {
        self.close().unwrap();
    }
}
