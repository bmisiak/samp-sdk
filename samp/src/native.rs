//! Natives as plain functions, axum-style: each parameter declares what it
//! extracts from the call, and any function whose parameters are extractable
//! is a [`Native`].
//!
//! A parameter is one of:
//! - a [`FromAmxCell`] type (`i32`, `bool`, `f32`, `AmxString`, `Ref<T>`, …)
//!   — decoded from the next argument cell;
//! - [`Amx`] — the current VM handle, consumes no argument, any position;
//! - [`VariadicArgs`] — the rest of the argument list, last position only
//!   (enforced by construction: no [`NativeParam`] impl exists for it, only
//!   the tail slot of the [`Native`] impls accepts it).
//!
//! The return type is any [`NativeReturn`]: a plain cell-convertible value
//! or a `Result` of one with a `Display` error, which is logged and turned
//! into 0 for PAWN.
//!
//! [`FromAmxCell`]: ../cell/trait.FromAmxCell.html
//! [`Amx`]: ../amx/struct.Amx.html
//! [`VariadicArgs`]: ../args/struct.VariadicArgs.html
//! [`NativeReturn`]: ../plugin/trait.NativeReturn.html
use std::marker::PhantomData;
use std::ptr::NonNull;

use samp_sdk::args::{Args, VariadicArgs};
use samp_sdk::cell::FromAmxCell;
use samp_sdk::error::AmxResult;
use samp_sdk::raw::types::AMX;

use crate::amx::Amx;
use crate::plugin::{log_native_error, NativeReturn};

/// How one parameter of a native obtains its value.
///
/// `Marker` only disambiguates impls that would otherwise overlap in the
/// eyes of coherence (the blanket [`FromAmxCell`] impl vs the [`Amx`] impl);
/// it is always inferred, never written.
///
/// [`FromAmxCell`]: ../cell/trait.FromAmxCell.html
/// [`Amx`]: ../amx/struct.Amx.html
pub trait NativeParam<'amx, Marker>: Sized {
    fn extract(amx: Amx<'amx>, args: &mut Args<'amx>) -> AmxResult<Self>;
}

/// Marker: decoded from the next argument cell.
pub struct FromCell(());

/// Marker: the current [`Amx`], consuming no argument.
///
/// [`Amx`]: ../amx/struct.Amx.html
pub struct CurrentAmx(());

impl<'amx, T: FromAmxCell<'amx>> NativeParam<'amx, FromCell> for T {
    fn extract(_amx: Amx<'amx>, args: &mut Args<'amx>) -> AmxResult<Self> {
        args.next_arg()
    }
}

impl<'amx> NativeParam<'amx, CurrentAmx> for Amx<'amx> {
    fn extract(amx: Amx<'amx>, _args: &mut Args<'amx>) -> AmxResult<Self> {
        Ok(amx)
    }
}

/// A function callable as an AMX native: every parameter is a
/// [`NativeParam`] (plus at most one final [`VariadicArgs`]) and the return
/// type is a [`NativeReturn`].
///
/// `Marker` encodes the parameter list so the per-arity impls don't overlap;
/// it is always inferred.
///
/// [`VariadicArgs`]: ../args/struct.VariadicArgs.html
/// [`NativeReturn`]: ../plugin/trait.NativeReturn.html
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as an AMX native",
    note = "every parameter must be a `FromAmxCell` type, `Amx`, or a final `VariadicArgs`, \
            and the return type must be a cell-convertible value or a `Result` of one"
)]
pub trait Native<'amx, Marker> {
    /// Extract the arguments, call the function, and convert the return
    /// value, logging any failure under `name` and turning it into 0.
    fn invoke(self, name: &str, amx: Amx<'amx>, args: Args<'amx>) -> i32;
}

/// [`Native`] marker for functions whose arguments are all [`NativeParam`]s.
pub struct Exact<Params>(PhantomData<Params>);

/// [`Native`] marker for functions with a final [`VariadicArgs`] parameter.
///
/// [`VariadicArgs`]: ../args/struct.VariadicArgs.html
pub struct WithTail<Params>(PhantomData<Params>);

macro_rules! impl_native {
    ($($value:ident: $param:ident, $marker:ident);*) => {
        impl<'amx, Func, Ret, $($param, $marker,)*> Native<'amx, Exact<(Ret, $(($param, $marker),)*)>> for Func
        where
            Func: FnOnce($($param,)*) -> Ret,
            Ret: NativeReturn,
            $($param: NativeParam<'amx, $marker>,)*
        {
            fn invoke(self, name: &str, amx: Amx<'amx>, mut args: Args<'amx>) -> i32 {
                let _ = (&amx, &mut args);
                $(let $value = extract_or_return_zero!(name, amx, args, $param, $marker);)*
                finish(name, self($($value,)*))
            }
        }

        impl<'amx, Func, Ret, $($param, $marker,)*> Native<'amx, WithTail<(Ret, $(($param, $marker),)*)>> for Func
        where
            Func: FnOnce($($param,)* VariadicArgs<'amx>) -> Ret,
            Ret: NativeReturn,
            $($param: NativeParam<'amx, $marker>,)*
        {
            fn invoke(self, name: &str, amx: Amx<'amx>, mut args: Args<'amx>) -> i32 {
                let _ = (&amx, &mut args);
                $(let $value = extract_or_return_zero!(name, amx, args, $param, $marker);)*
                finish(name, self($($value,)* args.into_variadic()))
            }
        }
    };
}

macro_rules! extract_or_return_zero {
    ($name:ident, $amx:ident, $args:ident, $param:ident, $marker:ident) => {{
        // The PAWN-side argument number, for scripters reading the log.
        let position = $args.count() - $args.remaining() + 1;
        match <$param as NativeParam<'amx, $marker>>::extract($amx, &mut $args) {
            Ok(value) => value,
            Err(error) => {
                log_native_error(
                    $name,
                    format_args!("couldn't decode argument {position}: {error}"),
                );
                return 0;
            }
        }
    }};
}

fn finish(name: &str, returned: impl NativeReturn) -> i32 {
    match returned.into_return() {
        Ok(cell) => cell,
        Err(error) => {
            log_native_error(name, error);
            0
        }
    }
}

impl_native!();
impl_native!(a: A, MA);
impl_native!(a: A, MA; b: B, MB);
impl_native!(a: A, MA; b: B, MB; c: C, MC);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD; e: E, ME);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD; e: E, ME; f: F, MF);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD; e: E, ME; f: F, MF; g: G, MG);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD; e: E, ME; f: F, MF; g: G, MG; h: H, MH);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD; e: E, ME; f: F, MF; g: G, MG; h: H, MH; i: I, MI);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD; e: E, ME; f: F, MF; g: G, MG; h: H, MH; i: I, MI; j: J, MJ);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD; e: E, ME; f: F, MF; g: G, MG; h: H, MH; i: I, MI; j: J, MJ; k: K, MK);
impl_native!(a: A, MA; b: B, MB; c: C, MC; d: D, MD; e: E, ME; f: F, MF; g: G, MG; h: H, MH; i: I, MI; j: J, MJ; k: K, MK; l: L, ML);

/// The complete body of a generated `extern "C"` native entry point:
/// NULL-check the server's pointers, brand the [`Amx`] for this call,
/// validate the argument list, and hand both to `scope` — a closure
/// monomorphized per native that calls [`Native::invoke`].
///
/// The brand lifetime is invented *inside* this function (by
/// [`enter`]), which is why `scope` is the seam: within it the lifetime is
/// concrete, so the native function resolves against [`Native`] without any
/// higher-ranked bound over user code.
///
/// # Safety
/// `amx` and `params` must be the pointers the server passed to the
/// native's `extern "C"` entry point, valid for the duration of this call.
///
/// [`Amx`]: ../amx/struct.Amx.html
/// [`enter`]: ../amx/fn.enter.html
pub unsafe fn native_entry(
    name: &str,
    amx: *mut AMX,
    params: *const i32,
    scope: impl for<'amx> FnOnce(Amx<'amx>, Args<'amx>) -> i32,
) -> i32 {
    let Some(amx) = NonNull::new(amx) else {
        return 0;
    };
    let Some(params) = NonNull::new(params.cast_mut()) else {
        return 0;
    };
    // SAFETY: the server is mid-call into this native on behalf of `amx`,
    // and `params` is that call's argument list.
    unsafe {
        crate::amx::enter(amx, |amx| {
            let args = match Args::new(amx, params) {
                Ok(args) => args,
                Err(error) => {
                    log_native_error(name, format_args!("invalid argument list: {error}"));
                    return 0;
                }
            };
            scope(amx, args)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use samp_sdk::args::{Args, VariadicArgs};
    use samp_sdk::cell::AmxString;

    use super::{native_entry, Native};
    use crate::amx::Amx;

    fn dangling_amx() -> Amx<'static> {
        // SAFETY: never dereferenced by the value-extraction paths under test.
        unsafe { Amx::new(NonNull::dangling(), NonNull::dangling()) }
    }

    fn args_over(cells: &mut [i32]) -> Args<'static> {
        cells[0] = i32::try_from((cells.len() - 1) * size_of::<i32>()).unwrap();
        let params = NonNull::new(cells.as_mut_ptr()).unwrap();
        unsafe { Args::new(dangling_amx(), params) }.unwrap()
    }

    #[test]
    fn plain_value_parameters_extract_in_order() {
        fn subtract(a: i32, b: i32) -> i32 {
            a - b
        }

        let args = args_over(&mut [0, 7, 5]);
        assert_eq!(Native::invoke(subtract, "Subtract", dangling_amx(), args), 2);
    }

    #[test]
    fn amx_extracts_anywhere_without_consuming_an_argument() {
        fn middle(a: i32, _amx: Amx, b: i32) -> i32 {
            a * 10 + b
        }

        let args = args_over(&mut [0, 3, 4]);
        assert_eq!(Native::invoke(middle, "Middle", dangling_amx(), args), 34);
    }

    #[test]
    fn parameterless_natives_are_natives_too() {
        fn nullary() -> i32 {
            41
        }

        let args = args_over(&mut [0]);
        assert_eq!(Native::invoke(nullary, "Nullary", dangling_amx(), args), 41);
    }

    /// The signatures the plugin actually uses — strings, results, and a
    /// variadic tail — all satisfy `Native`. Extraction of by-reference
    /// types needs a live AMX, so this is a compile-time check only.
    #[test]
    fn borrowing_and_variadic_signatures_satisfy_native() {
        fn assert_native<'amx, Marker, F: Native<'amx, Marker>>(_f: F) {}

        fn creating(
            _amx: Amx,
            _callback: AmxString,
            _interval: u32,
            _repeat: bool,
            _rest: VariadicArgs,
        ) -> Result<i32, std::fmt::Error> {
            Ok(0)
        }
        fn stringly(_text: AmxString) -> bool {
            true
        }

        assert_native(creating);
        assert_native(stringly);
    }

    /// The exact shape the `plugin!` macro will generate: an `extern "C"`
    /// trampoline whose body brands the AMX and resolves the fn item against
    /// `Native` at the brand lifetime conjured inside `native_entry`.
    #[test]
    fn generated_trampoline_shape_resolves_and_runs() {
        fn add3(a: i32, b: i32, c: i32) -> i32 {
            a + b + c
        }

        unsafe extern "C" fn trampoline(
            amx: *mut samp_sdk::raw::types::AMX,
            params: *const i32,
        ) -> i32 {
            unsafe {
                native_entry("Add3", amx, params, |amx, args| {
                    Native::invoke(add3, "Add3", amx, args)
                })
            }
        }

        // `enter` builds a real Amx from the registry + server exports, so
        // fabricate a server: an export table whose AmxExports slot holds a
        // (never dereferenced) non-null pointer.
        let mut fake_exports = [0usize; 32];
        fake_exports[16] = NonNull::<usize>::dangling().as_ptr() as usize; // ServerData::AmxExports
        unsafe { crate::interlayer::load(NonNull::new(fake_exports.as_mut_ptr()).unwrap()) };

        // Only the *address* of an AMX is needed; no value is ever read.
        let mut fake_amx = std::mem::MaybeUninit::<samp_sdk::raw::types::AMX>::uninit();
        let mut params = [3 * size_of::<i32>() as i32, 10, 20, 12];
        let result = unsafe { trampoline(fake_amx.as_mut_ptr(), params.as_mut_ptr()) };
        assert_eq!(result, 42);
    }
}

#[cfg(test)]
mod error_probe {
    use crate::native::Native;
    fn variadic_in_middle(_rest: samp_sdk::args::VariadicArgs, _x: i32) -> i32 { 0 }
    fn assert_native<'amx, M, F: Native<'amx, M>>(_f: F) {}
    #[test]
    fn probe() { assert_native(variadic_in_middle); }
}
