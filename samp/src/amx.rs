//! Core Amx types with additional functions.
pub use samp_sdk::amx::*;
use samp_sdk::raw::types::AMX;

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// All AMX instances that went through `AmxLoad`, so publics can be
    /// executed later on an `Amx` that wasn't kept around. SA-MP plugins run
    /// on the server's main thread.
    static REGISTRY: RefCell<HashMap<AmxIdent, Amx>> = RefCell::new(HashMap::new());
}

/// Get a copy of a loaded `Amx` handle by its `AmxIdent`.
///
/// # Example
/// ```
/// use samp::prelude::*;
/// use samp::exec_public;
/// # use samp::native;
/// # use samp::amx::AmxIdent;
/// # use std::cell::RefCell;
/// # use std::collections::HashMap;
///
/// thread_local! {
///     static SUBSCRIBERS: RefCell<HashMap<String, Vec<AmxIdent>>> =
///         RefCell::new(HashMap::new());
/// }
///
/// #[native(name = "SubscribeToEvent")]
/// fn subscribe(amx: &Amx, event_name: AmxString) -> AmxResult<bool> {
///     let event_name = event_name.to_string();
///     SUBSCRIBERS.with(|subs| {
///         subs.borrow_mut()
///             .entry(event_name)
///             .or_insert(vec![])
///             .push(amx.ident());
///     });
///
///     Ok(true)
/// }
///
/// fn publish(event_name: &str) {
///     SUBSCRIBERS.with(|subs| {
///         if let Some(idents) = subs.borrow().get(event_name) {
///             for ident in idents {
///                 if let Some(amx) = samp::amx::get(*ident) {
///                     let _ = exec_public!(amx, event_name);
///                 }
///             }
///         }
///     });
/// }
/// ```
#[inline]
pub fn get(ident: AmxIdent) -> Option<Amx> {
    REGISTRY.with(|registry| registry.borrow().get(&ident).copied())
}

pub(crate) fn insert(ptr: *mut AMX) -> Amx {
    let amx = Amx::new(ptr, crate::interlayer::amx_exports());
    REGISTRY.with(|registry| registry.borrow_mut().insert(ptr.into(), amx));
    amx
}

pub(crate) fn remove(ptr: *mut AMX) -> Option<Amx> {
    REGISTRY.with(|registry| registry.borrow_mut().remove(&ptr.into()))
}

/// An unique identifier of an `Amx` instance.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct AmxIdent {
    ident: usize,
}

impl From<*mut AMX> for AmxIdent {
    fn from(ptr: *mut AMX) -> AmxIdent {
        AmxIdent {
            ident: ptr as usize,
        }
    }
}

/// Extended functional of an `Amx`.
pub trait AmxExt {
    /// Get an identifier of an `Amx`.
    ///
    /// # Example
    /// ```
    /// use samp::prelude::*;
    /// # use samp::native;
    ///
    /// #[native(name = "A")]
    /// fn native_a(amx: &Amx) -> AmxResult<bool> {
    ///     let ident = amx.ident();
    ///     // now you can use ident to get this Amx later by samp::amx::get
    ///     Ok(true)
    /// }
    /// ```
    fn ident(&self) -> AmxIdent;
}

impl AmxExt for Amx {
    #[inline]
    fn ident(&self) -> AmxIdent {
        self.amx().as_ptr().into()
    }
}
