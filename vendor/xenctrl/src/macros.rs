macro_rules! last_error {
    ($self:expr, $ok:expr) => {
        unsafe {
            let err = ($self.libxenctrl.get_last_error)($self.handle.as_ptr());
            #[allow(non_upper_case_globals)]
            match (*err).code {
                xc_error_code::XC_ERROR_NONE => Ok($ok),
                code => {
                    let desc = ($self.libxenctrl.error_code_to_desc)(code as _);
                    Err(XcError::new(ffi::CStr::from_ptr(desc).to_str().unwrap()))
                }
            }
        }
    };
    ($self:expr, $ok:expr, $ret: expr) => {
        if $ret >= 0 {
            Ok($ok)
        } else {
            let err = ($self.libxenctrl.get_last_error)($self.handle.as_ptr());
            unsafe {
                let desc = ($self.libxenctrl.error_code_to_desc)((*err).code as _);
                Err(XcError::new(ffi::CStr::from_ptr(desc).to_str().unwrap()))
            }
        }
    };
}

macro_rules! __RING_SIZE {
    ($name1: ident, $name2: ident) => {
        unsafe {
            __RD32!(
                (($name2 as usize + $name1 as usize
                    - &mut (*$name1).ring[0] as *mut xenvmevent_sys::vm_event_sring_entry as usize)
                    / std::mem::size_of_val(&(*$name1).ring[0])) as u32
            )
        }
    };
}
macro_rules! __RD2 {
    ($name: expr) => {
        if $name as u32 & 0x00000002 != 0 {
            0x2
        } else {
            $name as u32 & 0x1
        }
    };
}
macro_rules! __RD4 {
    ($name: expr) => {
        if $name as u32 & 0x0000000c != 0 {
            __RD2!(($name) >> 2) << 2
        } else {
            __RD2!($name)
        }
    };
}
macro_rules! __RD8 {
    ($name: expr) => {
        if $name as u32 & 0x000000f0 != 0 {
            __RD4!(($name) >> 4) << 4
        } else {
            __RD4!($name)
        }
    };
}
macro_rules! __RD16 {
    ($name: expr) => {
        if $name as u32 & 0x0000ff00 != 0 {
            __RD8!(($name) >> 8) << 8
        } else {
            __RD8!($name)
        }
    };
}
macro_rules! __RD32 {
    ($name: expr) => {
        if $name as u32 & 0xffff0000 != 0 {
            __RD16!(($name) >> 16) << 16
        } else {
            __RD16!($name)
        }
    };
}

#[macro_export]
macro_rules! RING_HAS_UNCONSUMED_REQUESTS {
    ($name: ident) => {{
        let req = unsafe { (*($name.sring)).req_prod - $name.req_cons };
        let rsp = $name.nr_ents - ($name.req_cons - $name.rsp_prod_pvt);
        if req < rsp {
            req
        } else {
            rsp
        }
    }};
}

macro_rules! RING_PUSH_RESPONSES {
    ($name1: ident) => {
        unsafe {
            (*($name1.sring)).rsp_prod = $name1.rsp_prod_pvt;
        }
    };
}

macro_rules! RING_GET_REQUEST {
    ($name1: ident, $name2: ident) => {
        unsafe {
            let ring_slice =
                slice::from_raw_parts((*$name1.sring).ring.as_mut_ptr(), $name1.nr_ents as usize);
            ring_slice[($name2 & ($name1.nr_ents - 1)) as usize].req
        }
    };
}

macro_rules! RING_PUT_RESPONSE {
    ($name1: ident, $name2: ident, $name3: ident) => {
        unsafe {
            let ring_slice = slice::from_raw_parts_mut(
                (*$name1.sring).ring.as_mut_ptr(),
                $name1.nr_ents as usize,
            );
            ring_slice[($name2 & ($name1.nr_ents - 1)) as usize].rsp = $name3;
        }
    };
}
