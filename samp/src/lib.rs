//! samp is a tool to develop plugins for [samp](http://sa-mp.com) servers written in rust.
//!
//! # project structure
//! * `samp` is a glue between crates described below (that's what you need).
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
//! use samp::plugin;
//!
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
//! plugin! {
//!     natives: [c"TestNative" = my_native],
//!     process_tick: each_tick, // optional; enables PROCESS_TICK support
//!     load: {
//!         // setup block, runs in Load(): configure logging etc.
//!         println!("Plugin is loaded.");
//!     }
//! }
//! ```

pub mod amx;
#[doc(hidden)]
pub mod interlayer;
pub mod native;
pub mod plugin;

pub use samp_sdk::exec_public; // macro
pub use samp_sdk::{args, cell, consts, error, exports, raw};

/// Re-export of the [`log`] crate: `error!`/`info!`/… records land in the
/// server log via the logger installed by [`plugin!`].
///
/// [`log`]: https://docs.rs/log
/// [`plugin!`]: macro.plugin.html
pub use log;

/// Define the server entry points and register plain Rust functions as AMX
/// natives.
///
/// Native names are C string literals so registration cannot accidentally
/// omit the required trailing NUL. Hooks are optional, but when present must
/// appear in the order shown below.
///
/// ```ignore
/// samp::plugin! {
///     natives: [
///         c"CreateThing" = create,
///         c"DeleteThing" = delete,
///     ],
///     on_unload: on_unload,
///     on_amx_load: on_amx_load,
///     on_amx_unload: on_amx_unload,
///     process_tick: process_tick,
///     load: {
///         // setup performed after the SDK has installed its logger
///     }
/// }
/// ```
#[macro_export]
macro_rules! plugin {
    (
        natives: [$($name:literal = $native:path),* $(,)?],
        $(on_unload: $on_unload:path,)?
        $(on_amx_load: $on_amx_load:path,)?
        $(on_amx_unload: $on_amx_unload:path,)?
        $(process_tick: $process_tick:path,)?
        load: $load:block $(,)?
    ) => {
        #[no_mangle]
        pub extern "system" fn Supports() -> u32 {
            $crate::interlayer::supports(false $(|| {
                let _ = stringify!($process_tick);
                true
            })?)
        }

        #[no_mangle]
        pub unsafe extern "system" fn Load(server_data: *const usize) -> i32 {
            let Some(server_data) = std::ptr::NonNull::new(server_data.cast_mut()) else {
                return 0;
            };
            // SAFETY: `Load` receives the server export table directly from
            // the SA-MP plugin ABI.
            unsafe { $crate::interlayer::load(server_data) };
            $load
            1
        }

        #[no_mangle]
        pub extern "system" fn Unload() {
            $($on_unload();)?
        }

        #[no_mangle]
        pub unsafe extern "system" fn AmxLoad(amx: *mut $crate::raw::types::AMX) {
            let natives: &[$crate::raw::types::AMX_NATIVE_INFO] = &[$({
                unsafe extern "C" fn trampoline(
                    amx: *mut $crate::raw::types::AMX,
                    params: *const i32,
                ) -> i32 {
                    // SAFETY: this trampoline is installed in the native
                    // table, so the AMX calls it with its current VM and
                    // parameter array.
                    unsafe {
                        $crate::native::native_entry($name, amx, params, |amx, args| {
                            $crate::native::Native::invoke($native, $name, amx, args)
                        })
                    }
                }

                $crate::raw::types::AMX_NATIVE_INFO {
                    name: ($name).as_ptr(),
                    func: trampoline,
                }
            }),*];

            // SAFETY: `AmxLoad` receives the VM currently being loaded from
            // the SA-MP plugin ABI.
            unsafe { $crate::interlayer::amx_load(amx, natives) };
            $(
                if let Some(amx) = std::ptr::NonNull::new(amx) {
                    // SAFETY: the server is still inside `AmxLoad` for this VM.
                    unsafe { $crate::amx::enter(amx, |amx| $on_amx_load(amx)) };
                }
            )?
        }

        #[no_mangle]
        pub unsafe extern "system" fn AmxUnload(amx: *mut $crate::raw::types::AMX) {
            // Run the hook while the AMX is still registered, so handles
            // obtained from its lent `Amx` remain valid during the hook.
            $(
                if let Some(amx) = std::ptr::NonNull::new(amx) {
                    // SAFETY: the server is still inside `AmxUnload` for this VM.
                    unsafe { $crate::amx::enter(amx, |amx| $on_amx_unload(amx)) };
                }
            )?
            $crate::interlayer::amx_unload(amx);
        }

        $(
            #[no_mangle]
            pub extern "system" fn ProcessTick() {
                $process_tick();
            }
        )?
    };
}

pub mod prelude {
    //! Most used imports.
    pub use crate::amx::{Amx, AmxExt, AmxHandle};
    pub use crate::cell::{AmxString, Buffer, FromAmxCell, RawCell, Ref, ToAmxCell, UnsizedBuffer};
    pub use crate::error::AmxResult;
}
