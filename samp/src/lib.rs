//! samp is a tool to develop plugins for [samp](http://sa-mp.com) servers written in rust.
//!
//! # project structure
//! * `samp` is a glue between crates described below (that's what you need).
//! * `samp-codegen` generates raw `extern "C"` functions and does whole nasty job.
//! * `samp-sdk` contains all types to work with amx.
//!
//! # design
//! There is no plugin object: natives and lifecycle hooks are free functions,
//! and plugin state lives in `thread_local!` storage (SA-MP plugins run on
//! the server's main thread).
//!
//! # usage
//! * [install](https://rustup.rs) rust compiler (supports only `i686` os versions because of samp server arch).
//! * add in your `Cargo.toml` this:
//! ```toml
//! [lib]
//! crate-type = ["cdylib"] # or dylib
//!
//! [dependencies]
//! samp = "0.2"
//! ```
//! * write your first plugin
//!
//! # examples
//! * your `lib.rs` file
//! ```rust,no_run
//! use samp::prelude::*; // export most useful types
//! use samp::{native, initialize_plugin}; // codegen macros
//!
//! #[native(name = "TestNative")]
//! fn my_native(_amx: Amx, text: AmxString) -> AmxResult<bool> {
//!     let text = text.to_string_lossy(); // decode amx bytes into a rust string
//!     println!("rust plugin: {}", text);
//!
//!     Ok(true)
//! }
//!
//! fn each_tick() {
//!     // called by the server every tick
//! }
//!
//! initialize_plugin!(
//!     natives: [my_native],
//!     process_tick: each_tick, // optional; enables PROCESS_TICK support
//!     {
//!         // setup block, runs in Load(): configure logging etc.
//!         println!("Plugin is loaded.");
//!     }
//! );
//! ```

pub mod amx;
#[doc(hidden)]
pub mod interlayer;
pub mod plugin;

pub use samp_codegen::{initialize_plugin, native};
pub use samp_sdk::{args, cell, consts, error, exports, raw};
pub use samp_sdk::exec_public; // macros

/// Re-export of the [`log`] crate: `error!`/`info!`/… records land in the
/// server log via the logger installed by [`initialize_plugin!`].
///
/// [`log`]: https://docs.rs/log
/// [`initialize_plugin!`]: macro.initialize_plugin.html
pub use log;

pub mod prelude {
    //! Most used imports.
    pub use crate::amx::{Amx, AmxExt, AmxHandle};
    pub use crate::cell::{AmxString, Buffer, FromAmxCell, RawCell, Ref, ToAmxCell, UnsizedBuffer};
    pub use crate::error::AmxResult;
}
