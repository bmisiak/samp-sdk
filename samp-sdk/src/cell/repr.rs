//! A module to discribe how AMX cells work.
use crate::amx::Amx;
use crate::error::{AmxError, AmxResult};

/// AmxCell trait is a core trait of whole SDK.
/// It shows that value can be borrowed (or copied if it's a primitive) from AMX and passed to it.
pub trait AmxCell<'amx>
where
    Self: Sized,
{
    fn from_raw(_amx: Amx<'amx>, _cell: i32) -> AmxResult<Self>
    where
        Self: 'amx,
    {
        Err(AmxError::General)
    }

    /// Get a value which can be passed to AMX.
    fn as_cell(&self) -> i32;
}

/// Marker for argument readers that interpret the raw cell as an AMX
/// address and go through `amx_GetAddr` translation, as opposed to reading
/// the cell as the value itself.
///
/// PAWN passes declared references (`&x`), strings, arrays — and *every*
/// argument in a variadic segment (`{Float,_}:...`), even plain integers —
/// this way. [`VariadicArgs`] only accepts these types, so a variadic cell
/// cannot be misread as a by-value primitive.
///
/// [`VariadicArgs`]: ../../args/struct.VariadicArgs.html
pub trait AmxCellByRef<'amx>: AmxCell<'amx> {}

/// A marker showing that a value can be stored directly on a stack or a heap of an AMX.
///
/// Types: i8, u8, i16, u16, i32, u32, usize, isize, f32, bool
///
/// There is no values that's bigger than 4 bytes, because size of an AMX cell is 32 bits.
///
/// # Safety
/// Implementors must be plain `Copy` values that fit in a single 32-bit AMX
/// cell, because they are read from and written to raw cell memory as-is.
pub unsafe trait AmxPrimitive
where
    Self: Sized + Copy,
{
}

impl<'a, T: AmxCell<'a>> AmxCell<'a> for &'a T {
    fn as_cell(&self) -> i32 {
        (**self).as_cell()
    }
}

impl<'a, T: AmxCell<'a>> AmxCell<'a> for &'a mut T {
    fn as_cell(&self) -> i32 {
        (**self).as_cell()
    }
}

macro_rules! impl_for_primitive {
    ($type:ty) => {
        impl<'amx> AmxCell<'amx> for $type {
            fn from_raw(_amx: Amx<'amx>, cell: i32) -> AmxResult<Self> {
                Ok(cell as Self)
            }

            fn as_cell(&self) -> i32 {
                *self as i32
            }
        }

        unsafe impl AmxPrimitive for $type {}
    };
}

impl_for_primitive!(i8);
impl_for_primitive!(u8);
impl_for_primitive!(i16);
impl_for_primitive!(u16);
impl_for_primitive!(i32);
impl_for_primitive!(u32);
impl_for_primitive!(usize);
impl_for_primitive!(isize);

impl<'amx> AmxCell<'amx> for f32 {
    fn from_raw(_amx: Amx<'amx>, cell: i32) -> AmxResult<f32> {
        Ok(f32::from_bits(cell as u32))
    }

    fn as_cell(&self) -> i32 {
        // bitwise reinterpretation: `self as i32` would convert the value
        // (10.0 -> 10) instead of preserving the cell bits (0x41200000)
        self.to_bits() as i32
    }
}

impl<'amx> AmxCell<'amx> for bool {
    fn from_raw(_amx: Amx<'amx>, cell: i32) -> AmxResult<bool> {
        // just to be sure that boolean value will be correct I don't use there `std::mem::transmute` or `as` keyword.
        Ok(cell != 0)
    }

    fn as_cell(&self) -> i32 {
        i32::from(*self)
    }
}

unsafe impl AmxPrimitive for f32 {}
unsafe impl AmxPrimitive for bool {}
