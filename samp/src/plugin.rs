//! Plugin setup helpers.
//!
//! There is no plugin object and no trait to implement: natives are free
//! functions (see [`native`]) and lifecycle hooks are free functions passed
//! to [`initialize_plugin!`]. Keep plugin state in `thread_local!` storage —
//! SA-MP plugins run on the server's main thread.
//!
//! [`native`]: ../attr.native.html
//! [`initialize_plugin!`]: ../macro.initialize_plugin.html
use std::convert::Infallible;
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};

use samp_sdk::cell::AmxCell;

static DEFAULT_LOGGER: AtomicBool = AtomicBool::new(true);

/// What a `#[native]` function may return.
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
                Ok(AmxCell::as_cell(&self))
            }
        }
    )+};
}

// Enumerated rather than blanket over `T: AmxCell`: coherence would treat a
// blanket impl as overlapping with the `Result` impl below.
impl_native_return!(i8, u8, i16, u16, i32, u32, isize, usize, f32, bool);

impl<T: AmxCell<'static>, E: Display> NativeReturn for Result<T, E> {
    type Error = E;

    fn into_return(self) -> Result<i32, E> {
        self.map(|value| value.as_cell())
    }
}

/// Put a native's failure in the server log. Called by generated wrappers.
#[doc(hidden)]
pub fn log_native_error(native_name: &str, error: impl Display) {
    crate::interlayer::log(format_args!("{} error: {}", native_name, error));
}

/// Get a fern [`Dispatch`] that forwards log records to the server's
/// `logprintf`, and disable auto-installing the default logger.
///
/// # Example
/// ```rust,no_run
/// use samp::initialize_plugin;
///
/// use std::fs::OpenOptions;
///
/// initialize_plugin!({
///     // get a default samp logger (uses samp logprintf).
///     let samp_logger = samp::plugin::logger()
///         .level(log::LevelFilter::Warn); // logging only warn and error messages
///
///     let log_file = fern::log_file("myplugin.log").expect("Something wrong!");
///
///     // log trace and debug messages in an another file
///     let trace_level = fern::Dispatch::new()
///         .level(log::LevelFilter::Trace) // write ALL types of logs
///         .chain(log_file);
///
///     let _ = fern::Dispatch::new()
///         .format(|callback, message, record| {
///             // all messages will be formated like
///             // [MyPlugin][ERROR]: something (error!("something"))
///             // [MyPlugin][INFO]: some info (info!("some info"))
///             callback.finish(format_args!("[MyPlugin][{}]: {}", record.level(), message))
///         })
///         .chain(samp_logger)
///         .chain(trace_level)
///         .apply();
/// });
/// ```
///
/// [`Dispatch`]: https://docs.rs/fern/0.6/fern/struct.Dispatch.html
pub fn logger() -> fern::Dispatch {
    DEFAULT_LOGGER.store(false, Ordering::Relaxed);

    fern::Dispatch::new().chain(fern::Output::call(|record| {
        crate::interlayer::log(record.args());
    }))
}

/// Called by the generated `Load()` after the setup block: installs the
/// default logger unless the setup block built its own via [`logger`].
///
/// [`logger`]: fn.logger.html
#[doc(hidden)]
pub fn finish_setup() {
    if DEFAULT_LOGGER.load(Ordering::Relaxed) {
        let _ = logger().apply();
    }
}

/// The `amx_*` exports table passed by the server, used by generated natives
/// to construct [`Amx`] handles from raw pointers.
///
/// [`Amx`]: ../amx/struct.Amx.html
#[doc(hidden)]
pub fn amx_exports() -> usize {
    crate::interlayer::amx_exports()
}
