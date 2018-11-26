use crate::consts::PLUGIN_DATA_AMX_EXPORTS;
use crate::types::Logprintf;

use std::fmt::Display;
use std::ffi::CString;

pub struct InternalData {
    logprintf: usize,
    amx_functions: *const u32,
}

impl InternalData {
    pub fn new(data: *const *const u32) -> InternalData {
        unsafe {
            InternalData {
                logprintf: data.offset(0).read() as _,
                amx_functions: data.offset(PLUGIN_DATA_AMX_EXPORTS).read(),
            }
        }
    }

    pub fn log<T: Display>(&self, message: T) {
        unsafe {
            let log_fn: Logprintf = std::mem::transmute(self.logprintf);
            let result = CString::new(format!("{}", message))
                                .map(|cstring| log_fn(cstring.as_ptr()));

            if let Err(error) = result {
                println!("samp-sdk log error: {}", error);
            }
        }
    }

    pub fn amx_function(&self, offset: isize) -> *const u32 {
        unsafe {
            self.amx_functions.offset(offset)
        }
    }
}
