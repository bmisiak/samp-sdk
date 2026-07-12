//! String representation inside an AMX.
use std::ffi::CString;
use std::fmt;

use super::{AmxByRef, Buffer, FromAmxCell, RawCell, ToAmxCell, UnsizedBuffer};
use crate::amx::Amx;
use crate::error::AmxResult;

const MAX_UNPACKED: u32 = 0x00FF_FFFF;

fn is_packed(cell: RawCell) -> bool {
    cell.bits() > MAX_UNPACKED
}

/// A wrapper around an AMX string.
///
/// Like `CStr`, this is a zero-terminated *byte* string: PAWN strings carry
/// whatever bytes the server's locale produced (cp1252 chat, player names,
/// …) and are **not** UTF-8. There is deliberately no `Display`/`ToString`:
/// [`to_bytes`] and [`to_cstring`] return the truth, and the only path to a
/// Rust `String` is the explicitly named [`to_string_lossy`].
///
/// [`to_bytes`]: #method.to_bytes
/// [`to_cstring`]: #method.to_cstring
/// [`to_string_lossy`]: #method.to_string_lossy
pub struct AmxString<'amx> {
    inner: Buffer<'amx>,
    // real length of the string
    len: usize,
}

impl<'amx> AmxString<'amx> {
    /// Create a new AmxString from an allocated buffer and fill it with a string.
    ///
    /// # Panics
    /// Panics when the buffer is smaller than `bytes.len() + 1` cells (the
    /// string plus its zero terminator).
    pub fn new(buffer: Buffer<'amx>, bytes: &[u8]) -> AmxString<'amx> {
        assert!(
            buffer.len() > bytes.len(),
            "the buffer has no room for the string and its zero terminator"
        );

        for (idx, byte) in bytes.iter().enumerate() {
            buffer.set(idx, i32::from(*byte));
        }

        buffer.set(bytes.len(), 0);

        AmxString {
            len: bytes.len(),
            inner: buffer,
        }
    }

    /// Convert an AMX string to a `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut vec = Vec::with_capacity(self.len);
        if !self.inner.is_empty() && is_packed(self.inner.as_cells()[0].get()) {
            for index in 0..self.len {
                let bits = self.inner.as_cells()[index / size_of::<RawCell>()]
                    .get()
                    .bits();
                let shift = (size_of::<RawCell>() - 1 - index % size_of::<RawCell>()) * 8;
                let ch = (bits >> shift) as u8;
                if ch == b'\0' {
                    break;
                }
                vec.push(ch);
            }
        } else {
            for cell in &self.inner.as_cells()[..self.len] {
                vec.push(cell.get().get() as u8);
            }
        }

        vec
    }

    /// Copy the string into an owned [`CString`], e.g. for [`find_public`].
    ///
    /// PAWN strings are sequences of 32-bit cells, so a cell whose low byte
    /// is zero (e.g. the value 256) would embed an interior NUL; the copy
    /// stops there, mirroring what any C consumer of the bytes sees.
    ///
    /// [`CString`]: https://doc.rust-lang.org/std/ffi/struct.CString.html
    /// [`find_public`]: ../../amx/struct.Amx.html#method.find_public
    pub fn to_cstring(&self) -> CString {
        let mut bytes = self.to_bytes();
        if let Some(nul) = bytes.iter().position(|byte| *byte == 0) {
            bytes.truncate(nul);
        }
        CString::new(bytes).expect("interior NUL bytes were truncated above")
    }

    /// Decode the AMX string into a Rust `String`.
    ///
    /// AMX strings are raw bytes in the server's locale encoding, not UTF-8;
    /// bytes that don't form valid UTF-8 are replaced with U+FFFD.
    ///
    /// # Example
    /// ```
    /// use samp_sdk::cell::AmxString;
    /// # use samp_sdk::amx::Amx;
    /// # use samp_sdk::error::AmxResult;
    /// #
    /// # fn current_date() -> String {
    /// #       String::from("Today")
    /// # }
    ///
    /// fn log_error(amx: Amx, text: AmxString) -> AmxResult<bool> {
    ///     let string = text.to_string_lossy();
    ///     println!("[{}] PluginName error: {}", current_date(), string);
    ///
    ///     Ok(true)
    /// }
    /// ```
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.to_bytes()).into_owned()
    }

    /// Return a length of a string.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the number of cells occupied by the string and its terminator.
    pub fn cells_len(&self) -> usize {
        self.inner.len()
    }
}

impl<'amx> FromAmxCell<'amx> for AmxString<'amx> {
    fn from_cell(amx: Amx<'amx>, cell: RawCell) -> AmxResult<Self> {
        let buffer = UnsizedBuffer::from_cell(amx, cell)?;
        let ptr = buffer.as_ptr();
        let str_len = amx.strlen(ptr)?;
        let terminated_len = str_len
            .checked_add(1)
            .ok_or(crate::error::AmxError::Domain)?;
        let buf_len = if is_packed(buffer.first_cell()) {
            terminated_len.div_ceil(size_of::<RawCell>())
        } else {
            terminated_len
        };

        Ok(AmxString {
            inner: buffer.into_sized_buffer(buf_len)?,
            len: str_len,
        })
    }
}

impl ToAmxCell for AmxString<'_> {
    fn to_cell(&self) -> RawCell {
        self.inner.to_cell()
    }
}

impl<'amx> AmxByRef<'amx> for AmxString<'amx> {}

// No `Display` on purpose: it would hand out a lossy conversion through the
// innocent-looking auto-implemented `.to_string()`. `Debug` escapes instead.
impl fmt::Debug for AmxString<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "\"{}\"", self.to_bytes().escape_ascii())
    }
}

/// Fill a buffer with given string.
///
/// # Example
/// ```rust,no_run
/// use samp_sdk::cell::Buffer;
/// use samp_sdk::cell::string;
/// # use samp_sdk::error::AmxResult;
/// # use samp_sdk::amx::Amx;
///
/// # fn main() -> AmxResult<()> {
/// # let amx = unsafe { Amx::new(std::ptr::NonNull::dangling(), std::ptr::NonNull::dangling()) };
/// // let amx = ...;
/// let allocator = amx.allocator();
/// let buffer = allocator.allot_buffer(25)?; // let's think that we got a buffer from a native function input.
/// let string = "Hello, world!".to_string();
/// string::put_in_buffer(&buffer, &string)?; // store string in the AMX heap.
///
///
/// #   Ok(())
/// # }
/// ```
/// # Errors
/// Return `AmxError::General` when length of string bytes is more than size of the buffer.
pub fn put_in_buffer(buffer: &Buffer, string: &str) -> AmxResult<()> {
    let bytes = string.as_bytes();

    if bytes.len() >= buffer.len() {
        return Err(crate::error::AmxError::General);
    }

    for (idx, byte) in bytes.iter().enumerate() {
        buffer.set(idx, i32::from(*byte));
    }

    buffer.set(bytes.len(), 0);

    Ok(())
}
