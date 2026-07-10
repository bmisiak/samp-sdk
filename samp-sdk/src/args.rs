//! Workaround to parse input of natives functions.
use crate::amx::Amx;
use crate::cell::{AmxCell, AmxCellByRef};

/// A wrapper of a list of arguments of a native function.
pub struct Args<'a> {
    amx: Amx<'a>,
    args: *const i32,
    offset: usize,
}

impl<'a> Args<'a> {
    /// Creates a list from [`Amx`] and arguments.
    ///
    /// # Example
    /// ```
    /// use samp_sdk::args::Args;
    /// use samp_sdk::amx::Amx;
    /// use samp_sdk::cell::AmxString;
    /// # use samp_sdk::raw::types::AMX;
    ///
    /// // native: RawNative(const say_that[]);
    /// extern "C" fn raw_native(amx: *mut AMX, args: *mut i32) -> i32 {
    ///     # let amx_exports = 0;
    ///     // let amx_exports = ...;
    ///     let amx = unsafe { Amx::new(amx, amx_exports) };
    ///     let mut args = Args::new(amx, args);
    ///
    ///     let say_what = match args.next_arg::<AmxString>() {
    ///         Some(string) => string.to_string_lossy(),
    ///         None => {
    ///             println!("RawNative error: no argument");
    ///             return 0;
    ///         }
    ///     };
    ///
    ///     println!("RawNative: {}", say_what);
    ///
    ///     return 1;
    /// }
    /// ```
    ///
    /// [`Amx`]: ../amx/struct.Amx.html
    pub fn new(amx: Amx<'a>, args: *const i32) -> Args<'a> {
        Args {
            amx,
            args,
            offset: 0,
        }
    }

    /// Return the next argument in the list (like an iterator).
    ///
    /// When there is no arguments left returns `None`.
    ///
    /// # Choosing `T`: by-value vs by-reference
    /// The cells in the args array are whatever the PAWN compiler put there,
    /// and this method cannot verify your choice of `T` against the native's
    /// PAWN declaration — a wrong choice compiles fine and reads garbage.
    ///
    /// * Declared value parameters (`Fn(x)`, `Fn(Float:x)`) are passed by
    ///   value: read them as `i32`, `f32`, `bool`, `usize`.
    /// * Declared references (`Fn(&x)`), strings and arrays are passed as AMX
    ///   addresses: read them as `Ref<T>`, `AmxString`, `UnsizedBuffer`.
    /// * For the variadic tail (`{Float,_}:...`) switch to a typed cursor
    ///   with [`into_variadic`], which only compiles with by-reference reads.
    ///
    /// [`into_variadic`]: #method.into_variadic
    pub fn next_arg<T: AmxCell<'a> + 'a>(&mut self) -> Option<T> {
        let result = self.get(self.offset);
        self.offset += 1;

        result
    }

    /// Get an argument by position, if there is no argument in given location, returns `None`.
    ///
    /// # Example
    /// ```
    /// use samp_sdk::args::Args;
    /// use samp_sdk::amx::Amx;
    /// use samp_sdk::cell::Ref;
    /// # use samp_sdk::raw::types::AMX;
    ///
    /// // native: NativeFn(player_id, &Float:health, &Float:armor);
    /// extern "C" fn raw_native(amx: *mut AMX, args: *mut i32) -> i32 {
    ///     # let amx_exports = 0;
    ///     // let amx_exports = ...;
    ///     let amx = unsafe { Amx::new(amx, amx_exports) };
    ///     let args = Args::new(amx, args);
    ///
    ///     // change only armor
    ///     args.get::<Ref<f32>>(2)
    ///         .map(|armor| armor.set(255.0));
    ///
    ///     return 1;
    /// }
    /// ```
    pub fn get<T: AmxCell<'a> + 'a>(&self, offset: usize) -> Option<T> {
        if offset >= self.count() {
            return None;
        }

        unsafe { T::from_raw(self.amx, self.args.add(offset + 1).read()).ok() }
    }

    /// Reset a read offset for the [`next_arg()`] method.
    ///
    /// [`next_arg()`]: #method.next_arg
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Get count of arguments in the list.
    pub fn count(&self) -> usize {
        unsafe { (self.args.read() / 4) as usize }
    }

    /// Get count of arguments not yet consumed by [`next_arg()`].
    ///
    /// [`next_arg()`]: #method.next_arg
    pub fn remaining(&self) -> usize {
        self.count().saturating_sub(self.offset)
    }

    /// Treat everything from the current position onward as a variadic tail
    /// (`{Float,_}:...`).
    ///
    /// PAWN passes every argument in a variadic segment by reference — even
    /// plain integers and floats — so the returned cursor only reads types
    /// that go through AMX address translation. Asking it for a by-value
    /// primitive is a compile error instead of silently reading an address
    /// as a value.
    pub fn into_variadic(self) -> VariadicArgs<'a> {
        VariadicArgs { inner: self }
    }
}

/// A cursor over the variadic tail of a native's arguments, obtained from
/// [`Args::into_variadic`].
///
/// Every cell in a variadic segment holds an AMX address, so [`next_arg`] only
/// accepts [`AmxCellByRef`] readers: `Ref<i32>` for a cell, `Ref<f32>` for a
/// float, [`AmxString`] for a string, [`UnsizedBuffer`] for an array.
///
/// Reading a variadic cell as a by-value primitive would silently yield the
/// address instead of the value, so it does not compile:
///
/// ```compile_fail
/// use samp_sdk::args::Args;
///
/// fn broken(args: Args) {
///     let mut varargs = args.into_variadic();
///     let _: Option<i32> = varargs.next_arg(); // must be Ref<i32>
/// }
/// ```
///
/// [`Args::into_variadic`]: struct.Args.html#method.into_variadic
/// [`next_arg`]: #method.next_arg
/// [`AmxCellByRef`]: ../cell/repr/trait.AmxCellByRef.html
/// [`AmxString`]: ../cell/string/struct.AmxString.html
/// [`UnsizedBuffer`]: ../cell/buffer/struct.UnsizedBuffer.html
pub struct VariadicArgs<'a> {
    inner: Args<'a>,
}

impl<'a> VariadicArgs<'a> {
    /// Return the next variadic argument, dereferenced via the AMX.
    ///
    /// When there is no arguments left returns `None`.
    pub fn next_arg<T: AmxCellByRef<'a> + 'a>(&mut self) -> Option<T> {
        self.inner.next_arg()
    }

    /// Get count of variadic arguments not yet consumed.
    pub fn remaining(&self) -> usize {
        self.inner.remaining()
    }
}
