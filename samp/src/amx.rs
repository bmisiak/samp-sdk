//! Core Amx types with additional functions.
pub use samp_sdk::amx::*;
use samp_sdk::raw::types::AMX;

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;

thread_local! {
    // Which AMXes (gamemode + each filterscript) are currently loaded, and
    // the generation their handles must match. A linear scan beats a hash
    // map at these counts. SA-MP plugins run on the server's main thread.
    static REGISTRY: RefCell<Vec<(*mut AMX, u64)>> = const { RefCell::new(Vec::new()) };
    static NEXT_GENERATION: Cell<u64> = const { Cell::new(0) };
}

fn generation_of(amx: *mut AMX) -> Option<u64> {
    REGISTRY.with(|registry| {
        registry
            .borrow()
            .iter()
            .find(|(loaded, _)| *loaded == amx)
            .map(|(_, generation)| *generation)
    })
}

/// A storable identity of a loaded AMX (the gamemode or a filterscript).
///
/// The "weak" counterpart of [`Amx`]: it grants no capabilities and may
/// outlive the script it refers to. Redeem it with [`with`], which verifies
/// the script is still loaded before lending a usable [`Amx`].
///
/// The generation distinguishes scripts that reuse a freed script's memory
/// address, so a stale handle can't act on an unrelated script.
///
/// [`with`]: fn.with.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AmxHandle {
    ptr: *mut AMX,
    generation: u64,
}

impl AmxHandle {
    /// A handle referring to no AMX: [`with`] always returns `None`.
    ///
    /// [`with`]: fn.with.html
    pub fn dangling() -> AmxHandle {
        AmxHandle {
            ptr: std::ptr::null_mut(),
            generation: u64::MAX,
        }
    }
}

/// Run `scope` with a usable [`Amx`] if the script behind `handle` is still
/// loaded, or return `None` if it has been unloaded since.
///
/// This is the only way to act on a stored handle, so "verify before use"
/// can't be skipped. The `for<'amx>` bound keeps the lent `Amx` (and
/// anything parsed from it) from escaping the scope.
///
/// # Example
/// ```
/// use samp::amx::AmxHandle;
///
/// fn call_back_into_script(script: AmxHandle) {
///     let called = samp::amx::with(script, |amx| {
///         amx.find_public(c"OnMyPluginEvent")
///             .and_then(|idx| amx.exec(idx))
///     });
///     if called.is_none() {
///         println!("that script was unloaded");
///     }
/// }
/// ```
///
/// The lent `Amx` cannot be stashed for later — that's what [`AmxHandle`]
/// is for:
///
/// ```compile_fail
/// use samp::amx::AmxHandle;
///
/// let mut stash = None;
/// samp::amx::with(AmxHandle::dangling(), |amx| stash = Some(amx));
/// ```
///
/// [`AmxHandle`]: struct.AmxHandle.html
pub fn with<R>(handle: AmxHandle, scope: impl for<'amx> FnOnce(Amx<'amx>) -> R) -> Option<R> {
    if generation_of(handle.ptr) != Some(handle.generation) {
        return None;
    }

    // SAFETY: registered right now, and the server only unloads AMXes
    // between the plugin entry points that maintain the registry.
    let amx = unsafe { Amx::new(handle.ptr, crate::interlayer::amx_exports()) };
    Some(scope(amx))
}

/// Lend a usable [`Amx`] for the duration of a server-initiated call into
/// the plugin. Called by the wrappers `#[native]` and `initialize_plugin!`
/// generate; `amx` must be the pointer the server just passed in.
pub fn enter<R>(amx: NonNull<AMX>, scope: impl for<'amx> FnOnce(Amx<'amx>) -> R) -> R {
    // Natives can fire for an AMX that never went through AmxLoad (GDK).
    if generation_of(amx.as_ptr()).is_none() {
        register(amx.as_ptr());
    }

    // SAFETY: the server is mid-call into us on behalf of this AMX.
    let usable = unsafe { Amx::new(amx.as_ptr(), crate::interlayer::amx_exports()) };
    scope(usable)
}

pub(crate) fn register(amx: *mut AMX) {
    let generation = NEXT_GENERATION.with(|next| {
        let generation = next.get();
        next.set(generation + 1);
        generation
    });
    REGISTRY.with(|registry| registry.borrow_mut().push((amx, generation)));
}

pub(crate) fn unregister(amx: *mut AMX) {
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        if let Some(index) = registry.iter().position(|(loaded, _)| *loaded == amx) {
            registry.swap_remove(index);
        }
    });
}

/// Extended functional of an `Amx`.
pub trait AmxExt {
    /// Demote this usable `Amx` to a storable [`AmxHandle`].
    ///
    /// # Example
    /// ```
    /// use samp::prelude::*;
    /// use samp::amx::AmxHandle;
    /// # use samp::native;
    /// # use std::cell::Cell;
    ///
    /// thread_local! {
    ///     static LAST_CALLER: Cell<Option<AmxHandle>> = Cell::new(None);
    /// }
    ///
    /// #[native(name = "RememberMe")]
    /// fn remember_me(amx: Amx) -> AmxResult<bool> {
    ///     LAST_CALLER.with(|last| last.set(Some(amx.handle())));
    ///     // ...later, redeem it with samp::amx::with
    ///     Ok(true)
    /// }
    /// ```
    ///
    /// [`AmxHandle`]: struct.AmxHandle.html
    fn handle(&self) -> AmxHandle;
}

impl AmxExt for Amx<'_> {
    fn handle(&self) -> AmxHandle {
        let ptr = self.amx().as_ptr();
        let generation =
            generation_of(ptr).expect("a usable Amx exists only while its AMX is registered");

        AmxHandle { ptr, generation }
    }
}
