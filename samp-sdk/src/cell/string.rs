//! String interperation inside an AMX.
use std::ffi::CString;
use std::fmt;

use super::{AmxCell, Buffer, UnsizedBuffer};
use crate::amx::Amx;
use crate::error::AmxResult;
#[cfg(feature = "encoding")]
use crate::encoding;

const MAX_UNPACKED: i32 = 0x00FF_FFFF;

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
        // packed string
        if !self.inner.is_empty() && self.inner.get(0) > MAX_UNPACKED {
            let mut ptr = self.inner.as_ptr();
            let mut mark = 3;
            for _ in 0..self.len {
                let ch = (unsafe { *ptr } >> (mark * 8)) as u8;
                if ch == b'\0' {
                    break;
                }
                vec.push(ch);
                mark = (mark + 3) % 4;
                if mark == 3 {
                    ptr = unsafe { ptr.add(1) };
                }
            }
        } else {
            for cell in &self.inner.as_cells()[..self.len] {
                vec.push(cell.get() as u8);
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
    /// AMX strings are raw bytes in the server's locale encoding, not UTF-8.
    /// Without the `encoding` feature, bytes that don't form valid UTF-8 are
    /// replaced with U+FFFD; enable `encoding` to decode a configured code
    /// page (e.g. cp1251) instead.
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
        #[cfg(feature = "encoding")]
        return encoding::get().decode(&self.to_bytes()).0.into_owned();

        #[cfg(not(feature = "encoding"))]
        return String::from_utf8_lossy(&self.to_bytes()).into_owned();
    }

    /// Return a length of a string.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return a length of a buffer of a string
    pub fn bytes_len(&self) -> usize {
        self.inner.len()
    }
}

impl<'amx> AmxCell<'amx> for AmxString<'amx> {
    fn from_raw(amx: Amx<'amx>, cell: i32) -> AmxResult<AmxString<'amx>> {
        let buffer = UnsizedBuffer::from_raw(amx, cell)?;
        let ptr = buffer.as_ptr();
        let str_len = amx.strlen(ptr)?;
        let buf_len = str_len + 1;

        Ok(AmxString {
            inner: buffer.into_sized_buffer(buf_len)?,
            len: str_len,
        })
    }

    fn as_cell(&self) -> i32 {
        self.inner.as_cell()
    }
}

impl<'amx> super::repr::AmxCellByRef<'amx> for AmxString<'amx> {}

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
/// # let amx = unsafe { Amx::new(std::ptr::null_mut(), 0) };
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
    #[cfg(feature = "encoding")]
    let bytes = encoding::get().encode(string).0;

    #[cfg(not(feature = "encoding"))]
    let bytes = std::borrow::Cow::from(string.as_bytes());

    let bytes = bytes.as_ref();

    if bytes.len() >= buffer.len() {
        return Err(crate::error::AmxError::General);
    }

    for (idx, byte) in bytes.iter().enumerate() {
        buffer.set(idx, i32::from(*byte));
    }

    buffer.set(bytes.len(), 0);

    Ok(())
}
