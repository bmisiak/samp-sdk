//! Conversion between raw 32-bit PAWN cells and Rust values.

use std::num::{
    NonZeroI16, NonZeroI32, NonZeroI8, NonZeroIsize, NonZeroU16, NonZeroU32, NonZeroU8,
    NonZeroUsize,
};

use crate::amx::Amx;
use crate::error::{AmxError, AmxResult};

/// The untyped contents of one PAWN cell.
///
/// `RawCell` has exactly the layout of the AMX SDK's `cell`/Rust `i32`, but
/// does not choose an interpretation. Use [`get`] for the VM's signed value,
/// [`bits`] for an unsigned bit pattern, or [`FromAmxCell`] to perform a
/// checked semantic conversion.
///
/// [`get`]: #method.get
/// [`bits`]: #method.bits
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawCell(i32);

impl RawCell {
    pub const ZERO: Self = Self(0);

    /// Wrap the signed value used by the AMX ABI.
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    /// Return the cell as the VM's signed integer value.
    pub const fn get(self) -> i32 {
        self.0
    }

    /// Construct a cell by preserving all bits of an unsigned word.
    pub const fn from_bits(bits: u32) -> Self {
        Self(i32::from_ne_bytes(bits.to_ne_bytes()))
    }

    /// Return all 32 bits without a numeric signed-to-unsigned conversion.
    pub const fn bits(self) -> u32 {
        u32::from_ne_bytes(self.0.to_ne_bytes())
    }
}

impl From<i32> for RawCell {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<RawCell> for i32 {
    fn from(value: RawCell) -> Self {
        value.get()
    }
}

/// Decode a Rust value from one raw AMX cell.
///
/// Implementations validate semantic constraints. For example, unsigned
/// integers reject negative cells and `NonZeroUsize` also rejects zero.
pub trait FromAmxCell<'amx>: Sized {
    fn from_cell(amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self>;
}

/// Encode a Rust value into one AMX cell.
///
/// This trait is deliberately infallible, so it is only implemented for
/// types whose complete value range has an unambiguous cell representation.
/// Use [`RawCell::from_bits`] when deliberately passing an arbitrary `u32`
/// bit pattern.
pub trait ToAmxCell {
    fn to_cell(&self) -> RawCell;
}

/// Marker for native arguments whose cell contains an AMX address.
///
/// PAWN passes declared references, strings, arrays, and every variadic
/// argument by reference. [`VariadicArgs`] requires this marker so a stored
/// address cannot accidentally be parsed as a by-value integer.
///
/// [`VariadicArgs`]: ../../args/struct.VariadicArgs.html
pub trait AmxByRef<'amx>: FromAmxCell<'amx> {}

impl<'amx> FromAmxCell<'amx> for RawCell {
    fn from_cell(_amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self> {
        Ok(cell)
    }
}

impl ToAmxCell for RawCell {
    fn to_cell(&self) -> RawCell {
        *self
    }
}

impl<T: ToAmxCell + ?Sized> ToAmxCell for &T {
    fn to_cell(&self) -> RawCell {
        (**self).to_cell()
    }
}

impl<T: ToAmxCell + ?Sized> ToAmxCell for &mut T {
    fn to_cell(&self) -> RawCell {
        (**self).to_cell()
    }
}

macro_rules! impl_from_integer_cell {
    ($($type:ty),+ $(,)?) => {
        $(
            impl<'amx> FromAmxCell<'amx> for $type {
                fn from_cell(_amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self> {
                    Self::try_from(cell.get()).map_err(|_| AmxError::Domain)
                }
            }
        )+
    };
}

impl_from_integer_cell!(i8, u8, i16, u16, i32, u32, isize, usize);

macro_rules! impl_to_integer_cell {
    ($($type:ty),+ $(,)?) => {
        $(
            impl ToAmxCell for $type {
                fn to_cell(&self) -> RawCell {
                    RawCell::new(i32::from(*self))
                }
            }
        )+
    };
}

impl_to_integer_cell!(i8, u8, i16, u16, i32);

macro_rules! impl_from_non_zero_cell {
    ($($type:ty => $primitive:ty),+ $(,)?) => {
        $(
            impl<'amx> FromAmxCell<'amx> for $type {
                fn from_cell(_amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self> {
                    <$primitive>::try_from(cell.get())
                        .ok()
                        .and_then(Self::new)
                        .ok_or(AmxError::Domain)
                }
            }
        )+
    };
}

impl_from_non_zero_cell!(
    NonZeroI8 => i8,
    NonZeroU8 => u8,
    NonZeroI16 => i16,
    NonZeroU16 => u16,
    NonZeroI32 => i32,
    NonZeroU32 => u32,
    NonZeroIsize => isize,
    NonZeroUsize => usize,
);

macro_rules! impl_to_non_zero_cell {
    ($($type:ty),+ $(,)?) => {
        $(
            impl ToAmxCell for $type {
                fn to_cell(&self) -> RawCell {
                    RawCell::new(i32::from(self.get()))
                }
            }
        )+
    };
}

impl_to_non_zero_cell!(NonZeroI8, NonZeroU8, NonZeroI16, NonZeroU16, NonZeroI32);

impl<'amx> FromAmxCell<'amx> for f32 {
    fn from_cell(_amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self> {
        Ok(f32::from_bits(cell.bits()))
    }
}

impl ToAmxCell for f32 {
    fn to_cell(&self) -> RawCell {
        RawCell::from_bits(self.to_bits())
    }
}

impl<'amx> FromAmxCell<'amx> for bool {
    fn from_cell(_amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self> {
        Ok(cell != RawCell::ZERO)
    }
}

impl ToAmxCell for bool {
    fn to_cell(&self) -> RawCell {
        RawCell::new(i32::from(*self))
    }
}

#[cfg(test)]
mod tests {
    use super::{FromAmxCell, NonZeroU32, NonZeroUsize, RawCell, ToAmxCell};
    use crate::{amx::Amx, error::AmxError};

    fn parse<T: for<'amx> FromAmxCell<'amx>>(cell: i32) -> Result<T, AmxError> {
        let amx = unsafe { Amx::new(std::ptr::NonNull::dangling(), std::ptr::NonNull::dangling()) };
        T::from_cell(amx, RawCell::new(cell))
    }

    #[test]
    fn raw_bits_are_distinct_from_numeric_conversion() {
        assert_eq!(RawCell::new(-1).bits(), u32::MAX);
        assert_eq!(RawCell::from_bits(u32::MAX).get(), -1);
        assert!(matches!(parse::<u32>(-1), Err(AmxError::Domain)));
    }

    #[test]
    fn constrained_arguments_are_checked() {
        assert!(matches!(parse::<NonZeroU32>(-1), Err(AmxError::Domain)));
        assert!(matches!(parse::<NonZeroU32>(0), Err(AmxError::Domain)));
        assert!(matches!(parse::<NonZeroUsize>(0), Err(AmxError::Domain)));
        assert_eq!(parse::<NonZeroUsize>(1).unwrap().get(), 1);
    }

    #[test]
    fn floats_preserve_their_bits() {
        let value = -12.5_f32;
        assert_eq!(value.to_cell().bits(), value.to_bits());
        assert_eq!(parse::<f32>(value.to_cell().get()).unwrap(), value);
    }
}
