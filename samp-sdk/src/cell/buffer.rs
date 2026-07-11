//! Alias-safe views of AMX arrays.

use std::cell::Cell as StdCell;

use super::{AmxByRef, FromAmxCell, RawCell, Ref, ToAmxCell};
use crate::amx::Amx;
use crate::error::{AmxError, AmxResult};

/// A bounds-checked view of consecutive AMX cells.
///
/// Values are decoded and encoded one cell at a time. No Rust slice of a
/// caller-selected `T` is formed, so aliased arrays and reentrant script
/// writes remain safe.
pub struct Buffer<'amx> {
    inner: Ref<'amx, RawCell>,
    len: usize,
}

impl<'amx> Buffer<'amx> {
    pub(crate) fn new(reference: Ref<'amx, RawCell>, len: usize) -> Self {
        Self {
            inner: reference,
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// View the underlying storage without assigning it a semantic type.
    pub fn as_cells(&self) -> &[StdCell<RawCell>] {
        unsafe { std::slice::from_raw_parts(self.inner.as_ptr().cast(), self.len) }
    }

    /// Decode the cell at `index` as `T`.
    ///
    /// # Panics
    /// Panics when `index` is out of bounds.
    pub fn get<T: FromAmxCell<'amx>>(&self, index: usize) -> AmxResult<T> {
        T::from_cell(self.inner.amx(), self.as_cells()[index].get())
    }

    /// Encode `value` into the cell at `index`.
    ///
    /// # Panics
    /// Panics when `index` is out of bounds.
    pub fn set<T: ToAmxCell>(&self, index: usize, value: T) {
        self.as_cells()[index].set(value.to_cell());
    }

    /// Decode the whole buffer into owned Rust values.
    pub fn to_vec<T: FromAmxCell<'amx>>(&self) -> AmxResult<Vec<T>> {
        self.as_cells()
            .iter()
            .map(|cell| T::from_cell(self.inner.amx(), cell.get()))
            .collect()
    }

    /// Encode `values` into the start of the buffer.
    ///
    /// # Panics
    /// Panics when `values` is longer than the buffer.
    pub fn copy_from<T: ToAmxCell>(&self, values: &[T]) {
        for (cell, value) in self.as_cells()[..values.len()].iter().zip(values) {
            cell.set(value.to_cell());
        }
    }

    pub fn as_ptr(&self) -> *mut i32 {
        self.inner.as_ptr()
    }
}

impl ToAmxCell for Buffer<'_> {
    fn to_cell(&self) -> RawCell {
        self.inner.to_cell()
    }
}

impl std::fmt::Debug for Buffer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list()
            .entries(self.as_cells().iter().map(|cell| cell.get().get()))
            .finish()
    }
}

/// An AMX array address whose length must be supplied separately.
pub struct UnsizedBuffer<'amx> {
    inner: Ref<'amx, RawCell>,
    amx: Amx<'amx>,
}

impl<'amx> UnsizedBuffer<'amx> {
    pub(crate) fn first_cell(&self) -> RawCell {
        self.inner.raw()
    }

    /// Validate `len` against the AMX data section and create a sized view.
    ///
    /// The AMX does not retain source-language array lengths, so this proves
    /// only that the requested range stays inside the script's own memory.
    pub fn into_sized_buffer(self, len: usize) -> AmxResult<Buffer<'amx>> {
        if len > 0 {
            let last_cell = u32::try_from(len - 1)
                .ok()
                .and_then(|index| index.checked_mul(size_of::<RawCell>() as u32))
                .and_then(|offset| self.inner.address().checked_add_unsigned(offset))
                .ok_or(AmxError::MemoryAccess)?;
            self.amx.get_ref::<RawCell>(last_cell)?;
        }
        Ok(Buffer::new(self.inner, len))
    }

    pub fn as_ptr(&self) -> *mut i32 {
        self.inner.as_ptr()
    }
}

impl<'amx> FromAmxCell<'amx> for UnsizedBuffer<'amx> {
    fn from_cell(amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self> {
        Ok(Self {
            inner: amx.get_ref(cell.get())?,
            amx,
        })
    }
}

impl ToAmxCell for UnsizedBuffer<'_> {
    fn to_cell(&self) -> RawCell {
        self.inner.to_cell()
    }
}

impl<'amx> AmxByRef<'amx> for UnsizedBuffer<'amx> {}
