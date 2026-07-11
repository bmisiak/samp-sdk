//! Glue between the SA-MP server's raw plugin interface and safe code.
//! The functions here are called from the entry points that
//! `initialize_plugin!` generates.
use std::ffi::CString;
use std::fmt::Display;
use std::ptr::NonNull;
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

/// # Safety
/// `server_data` must be the server export table and remain valid until the
/// plugin unloads.
pub unsafe fn load(server_data: NonNull<usize>) {
    SERVER_EXPORTS.store(server_data.as_ptr() as usize, Ordering::Release);
}

fn server_exports() -> *const usize {
    let exports = SERVER_EXPORTS.load(Ordering::Acquire) as *const usize;
    assert!(
        !exports.is_null(),
        "the SA-MP server exports are not available before Load()"
    );
    exports
}

pub fn amx_exports() -> NonNull<usize> {
    let exports = unsafe {
        server_exports()
            .offset(ServerData::AmxExports.into())
            .read()
    } as *mut usize;
    NonNull::new(exports).expect("the server supplied a null AMX export table")
}

fn logprintf() -> Logprintf {
    unsafe {
        (server_exports().offset(ServerData::Logprintf.into()) as *const Logprintf).read()
    }
}

pub(crate) fn log<T: Display>(message: T) {
    if let Ok(cstr) = CString::new(message.to_string()) {
        // SAFETY: `Load` installed the server's `logprintf`; both strings
        // remain alive and NUL-terminated, and the fixed format consumes the
        // one supplied pointer.
        unsafe { logprintf()(c"%s".as_ptr(), cstr.as_ptr()) };
    }
}

/// # Safety
/// `amx` must be the VM currently being loaded by the server.
pub unsafe fn amx_load(amx: *mut AMX, natives: &[AMX_NATIVE_INFO]) {
    let Some(amx) = std::ptr::NonNull::new(amx) else {
        return;
    };
    // SAFETY: this is called only from the server's AmxLoad entrypoint.
    unsafe {
        crate::amx::enter(amx, |amx| {
            let _ = amx.register(natives); // registration raises its own AMX errors
        });
    }
}

pub fn amx_unload(amx: *mut AMX) {
    crate::amx::unregister(amx);
}
