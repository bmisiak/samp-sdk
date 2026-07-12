//! Glue between the SA-MP server's raw plugin interface and safe code.
//! The functions here are called from the entry points that
//! `initialize_plugin!` generates.
use std::fmt;
use std::os::raw::{c_char, c_int};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::thread::{self, ThreadId};

use samp_sdk::consts::{ServerData, Supports};
use samp_sdk::raw::functions::Logprintf;
use samp_sdk::raw::types::{AMX, AMX_NATIVE_INFO};

/// The server's export table, set once in `Load()` before anything else runs.
static SERVER_EXPORTS: AtomicUsize = AtomicUsize::new(0);

/// The thread `Load()` ran on — the server's main thread, the only one
/// allowed to touch `logprintf` and the AMX.
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

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
    let _ = MAIN_THREAD.set(thread::current().id());
    SERVER_EXPORTS.store(server_data.as_ptr() as usize, Ordering::Release);
    crate::plugin::install_logger();
}

pub(crate) fn on_main_thread() -> bool {
    MAIN_THREAD.get() == Some(&thread::current().id())
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

pub(crate) fn log(message: fmt::Arguments) {
    // The server's logprintf vsprintf's into a fixed `char buffer[512]` with
    // no bounds checking — a longer message would overflow its stack, so
    // truncate to 511 bytes plus the NUL (at a char boundary, to not tear a
    // multi-byte character).
    const MAX_LINE: usize = 511;

    let text = message.to_string();
    let mut cutoff = text.len().min(MAX_LINE);
    while !text.is_char_boundary(cutoff) {
        cutoff -= 1;
    }
    let len = c_int::try_from(cutoff).expect("at most 511 bytes fits c_int");
    // SAFETY: `Load` installed the server's `logprintf`, which copies its
    // arguments synchronously. The `%.*s` precision bounds the read to `len`
    // initialized bytes of `text`, so no NUL terminator is needed.
    unsafe { logprintf()(c"%.*s".as_ptr(), len, text.as_ptr().cast::<c_char>()) };
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
