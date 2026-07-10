//! Glue between the SA-MP server's raw plugin interface and safe code.
//! The functions here are called from the entry points that
//! `initialize_plugin!` generates.
use std::ffi::CString;
use std::fmt::Display;
use std::sync::atomic::{AtomicUsize, Ordering};

use samp_sdk::consts::{ServerData, Supports};
use samp_sdk::raw::functions::Logprintf;
use samp_sdk::raw::types::{AMX, AMX_NATIVE_INFO};

/// The server's export table, set once in `Load()` before anything else runs.
static SERVER_EXPORTS: AtomicUsize = AtomicUsize::new(0);

pub fn supports(process_tick: bool) -> u32 {
    let mut supports = Supports::VERSION | Supports::AMX_NATIVES;

    if process_tick {
        supports |= Supports::PROCESS_TICK;
    }

    supports.bits()
}

pub fn load(server_data: *const usize) {
    SERVER_EXPORTS.store(server_data as usize, Ordering::Relaxed);
}

fn server_exports() -> *const usize {
    let exports = SERVER_EXPORTS.load(Ordering::Relaxed) as *const usize;
    assert!(
        !exports.is_null(),
        "the SA-MP server exports are not available before Load()"
    );
    exports
}

pub fn amx_exports() -> usize {
    unsafe {
        server_exports()
            .offset(ServerData::AmxExports.into())
            .read()
    }
}

fn logprintf() -> Logprintf {
    unsafe {
        (server_exports().offset(ServerData::Logprintf.into()) as *const Logprintf).read()
    }
}

pub(crate) fn log<T: Display>(message: T) {
    if let Ok(cstr) = CString::new(message.to_string()) {
        logprintf()(cstr.as_ptr());
    }
}

pub fn amx_load(amx: *mut AMX, natives: &[AMX_NATIVE_INFO]) {
    let Some(amx) = std::ptr::NonNull::new(amx) else {
        return;
    };
    crate::amx::enter(amx, |amx| {
        let _ = amx.register(natives); // don't care about errors, that function always raises errors.
    });
}

pub fn amx_unload(amx: *mut AMX) {
    crate::amx::unregister(amx);
}
