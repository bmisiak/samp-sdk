//! Different smart-pointers to work around raw AMX values.
use std::marker::PhantomData;

use crate::amx::Amx;
use crate::error::AmxResult;

pub mod buffer;
pub mod repr;
pub mod string;

pub use buffer::{Buffer, UnsizedBuffer};
pub use repr::{AmxCell, AmxCellByRef, AmxPrimitive};
pub use string::AmxString;

/// A handle to a cell in the [`Amx`].
///
/// # Why `get`/`set` instead of references
/// The pointee lives in AMX memory: the *script* decides what aliases it
/// (`MyNative(x, x)` hands a native two `Ref`s to one cell), and PAWN code
/// may write to it whenever an `exec` runs. The Rust compiler can't see
/// either, so handing out `&T`/`&mut T` here would let safe code create
/// undefined behavior. Instead this type copies values in and out through
/// raw pointers, like [`Cell`] — aliased handles and reentrant writes are
/// then just fine.
///
/// [`Amx`]: ../amx/struct.Amx.html
/// [`Cell`]: https://doc.rust-lang.org/std/cell/struct.Cell.html
pub struct Ref<'amx, T: Sized + AmxPrimitive> {
    amx_addr: i32,
    phys_addr: *mut T,
    marker: PhantomData<&'amx ()>,
}

impl<'amx, T: Sized + AmxPrimitive> Ref<'amx, T> {
    /// Create a new wrapper over an AMX cell.
    ///
    /// # Safety
    /// `phys_addr` must point into the data section of a loaded AMX and stay
    /// valid for `'amx`.
    ///
    /// It's not recomended to use directly, instead get a reference from [`Args`] or [`Amx::get_ref`].
    ///
    /// [`Args`]: ../args/struct.Args.html
    /// [`Amx::get_ref`]: ../amx/struct.Amx.html#method.get_ref
    pub unsafe fn new(amx_addr: i32, phys_addr: *mut T) -> Ref<'amx, T> {
        Ref {
            amx_addr,
            phys_addr,
            marker: PhantomData,
        }
    }

    /// Get an inner AMX address to cell (not physical).
    ///
    /// # Example
    /// ```
    /// # use samp_sdk::amx::Amx;
    /// use samp_sdk::cell::Ref;
    /// fn native_fn(amx: Amx, arg: Ref<usize>) {
    ///     let cell_addr = arg.address();
    ///     println!("The argument stored in the {} cell.", cell_addr);
    /// }
    /// ```
    #[inline]
    pub fn address(&self) -> i32 {
        self.amx_addr
    }

    /// Get a pointer to the memory cell.
    #[inline]
    pub fn as_ptr(&self) -> *mut T {
        self.phys_addr
    }

    /// Read the current value of the cell.
    ///
    /// # Example
    /// ```
    /// # use samp_sdk::amx::Amx;
    /// # use samp_sdk::cell::Ref;
    /// # use samp_sdk::error::AmxResult;
    /// // native: SetPlayerArmour(playerid, &Float:armour);
    /// fn read_armour(amx: Amx, armour: Ref<f32>) -> AmxResult<f32> {
    ///     Ok(armour.get())
    /// }
    /// ```
    #[inline]
    pub fn get(&self) -> T {
        unsafe { self.phys_addr.read() }
    }

    /// Write a value into the cell.
    ///
    /// Takes `&self` because this is interior mutability: other handles (or
    /// the script itself) may point at the same cell.
    #[inline]
    pub fn set(&self, value: T) {
        unsafe { self.phys_addr.write(value) }
    }
}

impl<'amx, T: Sized + AmxPrimitive> AmxCell<'amx> for Ref<'amx, T> {
    fn from_raw(amx: Amx<'amx>, cell: i32) -> AmxResult<Ref<'amx, T>> {
        amx.get_ref(cell)
    }

    fn as_cell(&self) -> i32 {
        self.address()
    }
}

impl<'amx, T: Sized + AmxPrimitive> repr::AmxCellByRef<'amx> for Ref<'amx, T> {}
