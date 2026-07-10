//! Contains types to interact with AMX arrays.
use std::cell::Cell;

use super::{AmxCell, Ref};
use crate::amx::Amx;
use crate::error::{AmxError, AmxResult};

/// A handle to a sequence of `Amx` cells.
///
/// # Why there is no `&[i32]` / `&mut [i32]` access
/// The cells live in AMX memory: the script decides what aliases them (it
/// can pass the same array to two parameters of one native) and may write
/// to them during any `exec`. Handing out slices would let safe code create
/// aliased `&mut` — undefined behavior the compiler cannot see. Values are
/// copied in and out instead; [`as_cells`] gives an in-place view for
/// iteration, which stays sound because [`Cell`] permits aliased mutation.
///
/// # Example
/// ```
/// use samp_sdk::cell::{UnsizedBuffer, Buffer};
/// # use samp_sdk::amx::Amx;
/// # use samp_sdk::error::AmxResult;
///
/// // native: IGiveYouABuffer(buffer[], size);
/// fn it_gave_me_a_buffer(amx: Amx, buffer: UnsizedBuffer, size: usize) -> AmxResult<i32> {
///     let buffer: Buffer = buffer.into_sized_buffer(size)?;
///
///     println!("Got {:?}", buffer);
///
///     for cell in buffer.as_cells() {
///         cell.set(cell.get() * 2);
///     }
///
///     println!("Changed to {:?}", buffer);
///     Ok(1)
/// }
/// ```
///
/// [`as_cells`]: #method.as_cells
/// [`Cell`]: https://doc.rust-lang.org/std/cell/struct.Cell.html
pub struct Buffer<'amx> {
    inner: Ref<'amx, i32>,
    len: usize,
}

impl<'amx> Buffer<'amx> {
    /// Create a buffer from a reference to its first element.
    pub fn new(reference: Ref<'amx, i32>, len: usize) -> Buffer<'amx> {
        Buffer {
            inner: reference,
            len,
        }
    }

    /// The number of cells in the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// View the buffer as a slice of [`Cell`]s.
    ///
    /// `Cell<i32>` is layout-compatible with `i32`, and `Cell` allows the
    /// pointee to be mutated through aliased handles, so this view is sound
    /// even when the script passed overlapping arrays.
    ///
    /// [`Cell`]: https://doc.rust-lang.org/std/cell/struct.Cell.html
    #[inline]
    pub fn as_cells(&self) -> &[Cell<i32>] {
        unsafe { std::slice::from_raw_parts(self.inner.as_ptr().cast::<Cell<i32>>(), self.len) }
    }

    /// Read the cell at `index`.
    ///
    /// # Panics
    /// Panics when `index` is out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> i32 {
        self.as_cells()[index].get()
    }

    /// Write `value` into the cell at `index`.
    ///
    /// Takes `&self` because this is interior mutability: other handles (or
    /// the script itself) may point at the same cells.
    ///
    /// # Panics
    /// Panics when `index` is out of bounds.
    #[inline]
    pub fn set(&self, index: usize, value: i32) {
        self.as_cells()[index].set(value);
    }

    /// Copy the buffer's contents into a `Vec`.
    pub fn to_vec(&self) -> Vec<i32> {
        self.as_cells().iter().map(Cell::get).collect()
    }

    /// Copy `values` into the buffer, starting at the first cell.
    ///
    /// # Panics
    /// Panics when `values` is longer than the buffer.
    pub fn copy_from(&self, values: &[i32]) {
        let cells = &self.as_cells()[..values.len()];
        for (cell, value) in cells.iter().zip(values) {
            cell.set(*value);
        }
    }

    /// Get a pointer to the first cell of the buffer.
    #[inline]
    pub fn as_ptr(&self) -> *mut i32 {
        self.inner.as_ptr()
    }
}

// Buffer cannot be parsed
impl<'amx> AmxCell<'amx> for Buffer<'amx> {
    #[inline]
    fn as_cell(&self) -> i32 {
        self.inner.as_cell()
    }
}

impl std::fmt::Debug for Buffer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_list()
            .entries(self.as_cells().iter().map(Cell::get))
            .finish()
    }
}

/// It's more like a temorary buffer that comes from AMX when a native is calling.
///
/// # Example
/// ```
/// use samp_sdk::cell::UnsizedBuffer;
/// # use samp_sdk::amx::Amx;
/// # use samp_sdk::error::AmxResult;
///
/// fn null_my_array(amx: Amx, array: UnsizedBuffer, length: usize) -> AmxResult<u32> {
///     let array = array.into_sized_buffer(length)?;
///
///     for cell in array.as_cells() {
///         cell.set(0);
///     }
///
///     return Ok(1)
/// }
/// ```
pub struct UnsizedBuffer<'amx> {
    inner: Ref<'amx, i32>,
    amx: Amx<'amx>,
}

impl<'amx> UnsizedBuffer<'amx> {
    /// Convert `UnsizedBuffer` into `Buffer` with given length.
    ///
    /// The length typically arrives as another script-supplied argument, so
    /// it is not trusted: the last cell is bounds-checked against the AMX's
    /// data section (`AmxError::MemoryAccess` when it lies outside). A
    /// script can still pass a wrong length *within* its own data — the AMX
    /// stores no array sizes, so no API can detect that — but it can only
    /// ever read its own memory, never this process's.
    ///
    /// # Example
    /// ```
    /// use samp_sdk::cell::UnsizedBuffer;
    /// # use samp_sdk::amx::Amx;
    /// # use samp_sdk::error::AmxResult;
    ///
    /// fn push_ones(amx: Amx, array: UnsizedBuffer, length: usize) -> AmxResult<i32> {
    ///     let buffer = array.into_sized_buffer(length)?;
    ///
    ///     for cell in buffer.as_cells() {
    ///         cell.set(1);
    ///     }
    ///     Ok(1)
    /// }
    /// ```
    pub fn into_sized_buffer(self, len: usize) -> AmxResult<Buffer<'amx>> {
        if len > 0 {
            let last_cell = u32::try_from(len - 1)
                .ok()
                .and_then(|index| index.checked_mul(4))
                .and_then(|offset| self.inner.address().checked_add_unsigned(offset))
                .ok_or(AmxError::MemoryAccess)?;
            self.amx.get_ref::<i32>(last_cell)?;
        }
        Ok(Buffer::new(self.inner, len))
    }

    /// Get a pointer to the first cell of the buffer.
    #[inline]
    pub fn as_ptr(&self) -> *mut i32 {
        self.inner.as_ptr()
    }
}

impl<'amx> super::repr::AmxCellByRef<'amx> for UnsizedBuffer<'amx> {}

impl<'amx> AmxCell<'amx> for UnsizedBuffer<'amx> {
    fn from_raw(amx: Amx<'amx>, cell: i32) -> AmxResult<UnsizedBuffer<'amx>> {
        Ok(UnsizedBuffer {
            inner: amx.get_ref(cell)?,
            amx,
        })
    }

    #[inline]
    fn as_cell(&self) -> i32 {
        self.inner.as_cell()
    }
}
