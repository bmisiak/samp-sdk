//! Plugin setup helpers.
//!
//! There is no plugin object and no trait to implement: natives are free
//! functions (see [`native`]) and lifecycle hooks are free functions passed
//! to [`initialize_plugin!`]. Keep plugin state in `thread_local!` storage —
//! SA-MP plugins run on the server's main thread.
//!
//! [`native`]: ../attr.native.html
//! [`initialize_plugin!`]: ../macro.initialize_plugin.html
use std::sync::atomic::{AtomicBool, Ordering};

use samp_sdk::cell::AmxCell;

static DEFAULT_LOGGER: AtomicBool = AtomicBool::new(true);

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

#[doc(hidden)]
pub fn convert_return_value<T: AmxCell<'static>>(value: T) -> i32 {
    value.as_cell()
}
