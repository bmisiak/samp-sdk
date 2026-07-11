//! String encoding.

use std::sync::atomic::{AtomicPtr, Ordering};

use encoding_rs::Encoding;
pub use encoding_rs::{WINDOWS_1251, WINDOWS_1252};

static DEFAULT_ENCODING: AtomicPtr<Encoding> =
    AtomicPtr::new(WINDOWS_1252 as *const Encoding as *mut Encoding);

pub fn set_default_encoding(encoding: &'static Encoding) {
    DEFAULT_ENCODING.store(
        encoding as *const Encoding as *mut Encoding,
        Ordering::Release,
    );
}

pub(crate) fn get() -> &'static Encoding {
    let encoding = DEFAULT_ENCODING.load(Ordering::Acquire);
    // SAFETY: the atomic is initialized from a static Encoding and the only
    // setter accepts another static Encoding, so it is always live/non-null.
    unsafe { &*encoding }
}
