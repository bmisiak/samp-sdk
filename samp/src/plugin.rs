//! Plugin setup helpers.
//!
//! There is no plugin object and no trait to implement: natives are free
//! functions and lifecycle hooks are free functions passed to [`plugin!`].
//! Keep plugin state in `thread_local!` storage —
//! SA-MP plugins run on the server's main thread.
//!
//! [`plugin!`]: ../macro.plugin.html
use std::convert::Infallible;
use std::fmt::Display;
use std::sync::{PoisonError, RwLock};

use samp_sdk::cell::{RawCell, ToAmxCell};

/// What a native function may return.
///
/// Natives ultimately return one 32-bit cell to PAWN. Infallible natives
/// return a plain cell-convertible value (`i32`, `bool`, `f32`, …);
/// fallible ones return `Result<value, error>` with any `Display` error —
/// the generated wrapper logs the error to the server log and returns 0 to
/// PAWN. Errors never cross into the VM, so there is no need to squeeze
/// plugin failures into [`AmxError`]; use your own error enum.
///
/// [`AmxError`]: ../error/enum.AmxError.html
pub trait NativeReturn {
    type Error: Display;
    fn into_return(self) -> Result<i32, Self::Error>;
}

macro_rules! impl_native_return {
    ($($type:ty),+) => {$(
        impl NativeReturn for $type {
            type Error = Infallible;

            fn into_return(self) -> Result<i32, Infallible> {
                Ok(self.to_cell().get())
            }
        }
    )+};
}

// Enumerated rather than blanket over `T: ToAmxCell`: coherence would treat a
// blanket impl as overlapping with the `Result` impl below.
impl_native_return!(
    i8,
    u8,
    i16,
    u16,
    i32,
    std::num::NonZeroI8,
    std::num::NonZeroU8,
    std::num::NonZeroI16,
    std::num::NonZeroU16,
    std::num::NonZeroI32,
    f32,
    bool,
    RawCell
);

impl<T: ToAmxCell, E: Display> NativeReturn for Result<T, E> {
    type Error = E;

    fn into_return(self) -> Result<i32, E> {
        self.map(|value| value.to_cell().get())
    }
}

/// Put a native's failure in the server log. Called by generated wrappers.
#[doc(hidden)]
pub fn log_native_error(native_name: &std::ffi::CStr, error: impl Display) {
    let native_name = native_name.to_string_lossy();
    crate::interlayer::log(format_args!("{} error: {}", native_name, error));
}

/// A replacement logger installed via [`set_logger`]; `None` routes records
/// to the default server-log logger.
static CUSTOM_LOG: RwLock<Option<&'static dyn log::Log>> = RwLock::new(None);

/// Route [`log`] records to a custom logger instead of the default one,
/// e.g. to also write to a plugin-specific file. May be called from the
/// setup block (the usual place) or later.
pub fn set_logger(logger: &'static dyn log::Log) {
    *CUSTOM_LOG.write().unwrap_or_else(PoisonError::into_inner) = Some(logger);
}

/// The logger handed to `log::set_logger`: delegates to the [`set_logger`]
/// replacement if there is one, and otherwise forwards records to the
/// server's `logprintf`, prefixed with the emitting crate's module path and
/// the level: `[my_plugin] ERROR: something broke`.
///
/// SA-MP's `logprintf` is not thread-safe, so records from other threads
/// (the `log` facade accepts them from anywhere) go to stderr instead.
struct ServerLog;

impl log::Log for ServerLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        match *CUSTOM_LOG.read().unwrap_or_else(PoisonError::into_inner) {
            Some(custom) => custom.enabled(metadata),
            None => true,
        }
    }

    fn log(&self, record: &log::Record) {
        if let Some(custom) = *CUSTOM_LOG.read().unwrap_or_else(PoisonError::into_inner) {
            custom.log(record);
        } else if crate::interlayer::on_main_thread() {
            crate::interlayer::log(format_args!(
                "[{}] {}: {}",
                record.target(),
                record.level(),
                record.args()
            ));
        } else {
            eprintln!(
                "[{}] {}: {}",
                record.target(),
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Some(custom) = *CUSTOM_LOG.read().unwrap_or_else(PoisonError::into_inner) {
            custom.flush();
        }
    }
}

/// Called from `interlayer::load` before the setup block runs, so that the
/// block's own `info!`/`error!` calls already reach the server log.
pub(crate) fn install_logger() {
    if log::set_logger(&ServerLog).is_ok() {
        // Keep dependencies' trace!/debug! chatter out of the server log by
        // default; a setup block may raise this with log::set_max_level.
        log::set_max_level(log::LevelFilter::Info);
    }
}
