//! Typed, alias-safe handles to AMX cell storage.

use std::cell::Cell as StdCell;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::amx::Amx;
use crate::error::AmxResult;

pub mod buffer;
pub mod repr;
pub mod string;

pub use buffer::{Buffer, UnsizedBuffer};
pub use repr::{AmxByRef, FromAmxCell, RawCell, ToAmxCell};
pub use string::AmxString;

/// A typed view of one cell in AMX memory.
///
/// The storage always remains a [`RawCell`]. `T` only chooses how [`get`]
/// decodes it and how [`set`] encodes it, so `Ref<bool>`, `Ref<f32>`, and
/// pointer-sized Rust types never cast AMX memory to an incompatible layout.
/// Aliasing is represented with [`std::cell::Cell`], since the script may
/// pass the same location more than once or mutate it during reentrant calls.
///
/// [`get`]: #method.get
/// [`set`]: #method.set
pub struct Ref<'amx, T> {
    amx: Amx<'amx>,
    amx_addr: i32,
    storage: NonNull<StdCell<RawCell>>,
    marker: PhantomData<fn() -> T>,
}

impl<'amx, T> Ref<'amx, T> {
    /// Create a typed view over one translated AMX cell.
    ///
    /// # Safety
    /// `storage` must point to one live AMX cell belonging to `amx` and stay
    /// valid for `'amx`.
    pub(crate) unsafe fn new(amx: Amx<'amx>, amx_addr: i32, storage: NonNull<i32>) -> Self {
        Self {
            amx,
            amx_addr,
            storage: storage.cast(),
            marker: PhantomData,
        }
    }

    /// Return the AMX-relative address, not the physical process pointer.
    pub fn address(&self) -> i32 {
        self.amx_addr
    }

    /// Return the physical pointer expected by raw AMX SDK calls.
    pub fn as_ptr(&self) -> *mut i32 {
        self.storage.as_ptr().cast()
    }

    pub(crate) fn amx(&self) -> Amx<'amx> {
        self.amx
    }

    pub(crate) fn raw(&self) -> RawCell {
        unsafe { self.storage.as_ref() }.get()
    }

    pub(crate) fn set_raw(&self, value: RawCell) {
        unsafe { self.storage.as_ref() }.set(value);
    }
}

impl<'amx, T: FromAmxCell<'amx>> Ref<'amx, T> {
    /// Decode the cell's current value as `T`.
    pub fn get(&self) -> AmxResult<T> {
        T::from_cell(self.amx, self.raw())
    }
}

impl<T: ToAmxCell> Ref<'_, T> {
    /// Encode `value` into the cell.
    pub fn set(&self, value: T) {
        self.set_raw(value.to_cell());
    }
}

impl<'amx, T> FromAmxCell<'amx> for Ref<'amx, T> {
    fn from_cell(amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self> {
        amx.get_ref(cell.get())
    }
}

impl<T> ToAmxCell for Ref<'_, T> {
    fn to_cell(&self) -> RawCell {
        RawCell::new(self.address())
    }
}

impl<'amx, T> AmxByRef<'amx> for Ref<'amx, T> {}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use crate::{amx::Amx, error::AmxError};

    use super::Ref;

    fn reference<T>(storage: &mut i32) -> Ref<'_, T> {
        let amx = unsafe { Amx::new(NonNull::dangling(), NonNull::dangling()) };
        unsafe { Ref::new(amx, 0, NonNull::from(storage)) }
    }

    #[test]
    fn references_decode_raw_storage_instead_of_casting_it() {
        let mut storage = -1;
        assert!(reference::<bool>(&mut storage).get().unwrap());
        assert!(matches!(
            reference::<u32>(&mut storage).get(),
            Err(AmxError::Domain)
        ));
    }

    #[test]
    fn references_encode_back_into_one_raw_cell() {
        let mut storage = 0;
        reference::<f32>(&mut storage).set(-12.5);
        assert_eq!(
            u32::from_ne_bytes(storage.to_ne_bytes()),
            (-12.5_f32).to_bits()
        );
    }
}
