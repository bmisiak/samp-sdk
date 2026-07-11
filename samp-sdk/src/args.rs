//! Fallible decoding of native arguments.

use std::ptr::NonNull;

use crate::amx::Amx;
use crate::cell::{AmxByRef, FromAmxCell, RawCell};
use crate::error::{AmxError, AmxResult};

/// A cursor over the arguments of one native call.
pub struct Args<'amx> {
    amx: Amx<'amx>,
    cells: NonNull<RawCell>,
    count: usize,
    offset: usize,
}

impl<'amx> Args<'amx> {
    /// Validate and wrap the parameter array supplied to an AMX native.
    ///
    /// # Safety
    /// `params` must be the parameter pointer for the current call, with its
    /// byte-count header followed by that many live cells. The array must
    /// remain valid for `'amx`.
    pub unsafe fn new(amx: Amx<'amx>, params: NonNull<i32>) -> AmxResult<Self> {
        let byte_count =
            usize::try_from(unsafe { params.as_ptr().read() }).map_err(|_| AmxError::Params)?;
        if byte_count % size_of::<RawCell>() != 0 {
            return Err(AmxError::Params);
        }

        Ok(Self {
            amx,
            cells: unsafe { NonNull::new_unchecked(params.as_ptr().add(1).cast()) },
            count: byte_count / size_of::<RawCell>(),
            offset: 0,
        })
    }

    /// Decode the next argument.
    pub fn next_arg<T: FromAmxCell<'amx>>(&mut self) -> AmxResult<T> {
        let result = self.get(self.offset);
        self.offset += 1;
        result
    }

    /// Decode the argument at `offset` without moving the cursor.
    pub fn get<T: FromAmxCell<'amx>>(&self, offset: usize) -> AmxResult<T> {
        if offset >= self.count {
            return Err(AmxError::Params);
        }

        let cell = unsafe { self.cells.as_ptr().add(offset).read() };
        T::from_cell(self.amx, cell)
    }

    pub fn reset(&mut self) {
        self.offset = 0;
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn remaining(&self) -> usize {
        self.count.saturating_sub(self.offset)
    }

    /// Restrict the remaining cursor to address-decoding argument types.
    ///
    /// PAWN passes every argument in a variadic segment by reference, even
    /// plain integer and float values.
    pub fn into_variadic(self) -> VariadicArgs<'amx> {
        VariadicArgs { inner: self }
    }
}

/// A cursor over a PAWN variadic tail (`{Float,_}:...`).
pub struct VariadicArgs<'amx> {
    inner: Args<'amx>,
}

impl<'amx> VariadicArgs<'amx> {
    pub fn next_arg<T: AmxByRef<'amx>>(&mut self) -> AmxResult<T> {
        self.inner.next_arg()
    }

    pub fn remaining(&self) -> usize {
        self.inner.remaining()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::ptr::NonNull;

    use crate::{amx::Amx, error::AmxError};

    use super::Args;

    #[test]
    fn conversion_errors_are_preserved() {
        let mut raw_args = [3 * size_of::<i32>() as i32, -1, 0, 1];
        let amx = unsafe { Amx::new(NonNull::dangling(), NonNull::dangling()) };
        let params = NonNull::new(raw_args.as_mut_ptr()).unwrap();
        let mut args = unsafe { Args::new(amx, params) }.unwrap();

        assert!(matches!(
            args.next_arg::<NonZeroUsize>(),
            Err(AmxError::Domain)
        ));
        assert!(matches!(
            args.next_arg::<NonZeroUsize>(),
            Err(AmxError::Domain)
        ));
        assert_eq!(args.next_arg::<NonZeroUsize>().unwrap().get(), 1);
        assert!(matches!(args.next_arg::<i32>(), Err(AmxError::Params)));
    }

    #[test]
    fn malformed_byte_count_is_rejected() {
        let mut raw_args = [3];
        let amx = unsafe { Amx::new(NonNull::dangling(), NonNull::dangling()) };
        let params = NonNull::new(raw_args.as_mut_ptr()).unwrap();
        assert!(matches!(
            unsafe { Args::new(amx, params) },
            Err(AmxError::Params)
        ));
    }
}
