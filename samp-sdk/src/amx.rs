//! Core Amx types.
use crate::cell::{AmxCell, AmxPrimitive, AmxString, Buffer, Ref};
use crate::consts::{AmxExecIdx, AmxFlags};
use crate::error::AmxResult;
use crate::exports::*;
use crate::raw::types::{AMX, AMX_HEADER, AMX_NATIVE_INFO};

#[cfg(feature = "encoding")]
use crate::encoding;

use std::borrow::Cow;
use std::ffi::CStr;
use std::marker::PhantomData;
use std::ptr::NonNull;

macro_rules! amx_try {
    ($call:expr) => {
        let result = $call;

        if result > 0 {
            return Err(result.into());
        }
    };
}

/// A usable handle to a loaded AMX (a gamemode or filterscript).
///
/// `'amx` brands the handle with the window during which the AMX is known to
/// be loaded: the duration of a native/hook call, or of a registry-verified
/// scope such as `samp::amx::with`. Because of the brand it cannot be stored;
/// demote it to a storable identity (`samp::amx::AmxExt::handle`) instead and
/// redeem that later.
#[derive(Debug, Clone, Copy)]
pub struct Amx<'amx> {
    ptr: *mut AMX,
    fn_table: usize,
    brand: PhantomData<&'amx ()>,
}

impl<'amx> Amx<'amx> {
    /// Create an AMX wrapper from a raw pointer (passed by callbacks like
    /// `AmxLoad`) and the address of the server's `amx_*` exports table.
    ///
    /// # Safety
    /// The caller asserts the AMX stays loaded for all of `'amx`. Prefer the
    /// branded entry points (native parameters, `samp::amx::with`) which
    /// choose `'amx` correctly.
    pub unsafe fn new(ptr: *mut AMX, fn_table: usize) -> Amx<'amx> {
        Amx {
            ptr,
            fn_table,
            brand: PhantomData,
        }
    }

    /// Register a list of plugin natives functions.
    pub fn register(&self, natives: &[AMX_NATIVE_INFO]) -> AmxResult<()> {
        let register = Register::from_table(self.fn_table);
        let len = natives.len();
        let ptr = natives.as_ptr();

        amx_try!(register(self.ptr, ptr, len as i32));

        Ok(())
    }

    // `find_public`, `find_native` and `find_pubvar` take `&CStr`, not
    // `&str`: AMX symbol names are zero-terminated bytes, and accepting
    // UTF-8 here would invite lossy conversions from `AmxString` that
    // silently look up the wrong name. Use c"literals" or
    // `AmxString::to_cstring`.

    pub(crate) fn allot<T: Sized + AmxPrimitive>(&self, cells: usize) -> AmxResult<Ref<'amx, T>> {
        let allot = Allot::from_table(self.fn_table);

        let mut amx_addr = 0;
        let mut phys_addr = 0;

        amx_try!(allot(self.ptr, cells as i32, &mut amx_addr, &mut phys_addr));

        unsafe { Ok(Ref::new(amx_addr, phys_addr as *mut T)) }
    }

    /// Execs an AMX function.
    ///
    /// # Examples
    ///
    /// ```
    /// use samp_sdk::amx::Amx;
    ///
    /// fn log_player_money(amx: Amx) {
    ///     let index = amx.find_public(c"AntiCheat_GetPlayerMoney").unwrap();
    ///     amx.push(1); // a player with ID 1
    ///
    ///     match amx.exec(index) {
    ///         Ok(money) => println!("Player {} has {} money.", 1, money),
    ///         Err(err) => println!("Error: {:?}", err),
    ///     }
    /// }
    /// ```
    pub fn exec(&self, index: AmxExecIdx) -> AmxResult<i32> {
        let exec = Exec::from_table(self.fn_table);
        let mut retval = 0;

        amx_try!(exec(self.ptr, &mut retval, index.into()));

        Ok(retval)
    }

    /// Push arguments inside the closure, then immediately exec the function.
    ///
    /// The [`Allocator`] passed to the closure lives until `amx_Exec` returns,
    /// so allotted strings and arrays stay valid for the whole call and the
    /// heap frame is released right after it. If the closure fails, the AMX
    /// stack pointer and pending argument count are restored, so a partially
    /// pushed argument list can never leak into a later `exec`.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    ///
    /// # use samp_sdk::error::AmxResult;
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::null_mut(), 0) };
    /// // forward SomePublicFunc(player_id, message[]);
    /// let public_fn = amx.find_public(c"SomePublicFunc")?;
    ///
    /// let retval = amx.exec_with_args(public_fn, |allocator| {
    ///     allocator.amx().push(allocator.allot_string("hello")?)?;
    ///     allocator.amx().push(10)?;
    ///     Ok(())
    /// })?;
    /// #   Ok(())
    /// # }
    /// ```
    pub fn exec_with_args(
        &self,
        index: AmxExecIdx,
        push_args: impl FnOnce(&Allocator<'amx>) -> AmxResult<()>,
    ) -> AmxResult<i32> {
        // Raw field access: the AMX struct is owned by C code and other
        // handles to it exist, so no `&AMX`/`&mut AMX` is ever materialized.
        let (saved_stk, saved_paramcount) = unsafe { ((*self.ptr).stk, (*self.ptr).paramcount) };
        let allocator = self.allocator();

        match push_args(&allocator) {
            Ok(()) => self.exec(index),
            Err(err) => {
                unsafe {
                    (*self.ptr).stk = saved_stk;
                    (*self.ptr).paramcount = saved_paramcount;
                }
                Err(err)
            }
        }
    }

    /// Returns an index of a native by its name.
    ///
    /// # Examples
    /// See `find_public` and `exec` examples.
    pub fn find_native(&self, name: &CStr) -> AmxResult<i32> {
        let find_native = FindNative::from_table(self.fn_table);
        let mut index = -1;

        amx_try!(find_native(self.ptr, name.as_ptr(), &mut index));

        Ok(index)
    }

    /// Returns an index of a public by its name.
    ///
    /// # Examples
    ///
    /// ```
    /// use samp_sdk::amx::Amx;
    /// use samp_sdk::error::AmxResult;
    ///
    /// fn has_on_player_connect(amx: Amx) -> AmxResult<bool> {
    ///     let public_index = amx.find_public(c"OnPlayerConnect")?;
    ///     Ok(i32::from(public_index) >= 0)
    /// }
    /// ```
    pub fn find_public(&self, name: &CStr) -> AmxResult<AmxExecIdx> {
        let find_public = FindPublic::from_table(self.fn_table);
        let mut index = -1;

        amx_try!(find_public(self.ptr, name.as_ptr(), &mut index));

        Ok(AmxExecIdx::from(index))
    }

    /// Returns a handle to a public variable.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    /// # use samp_sdk::error::AmxResult;
    ///
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::null_mut(), 0) };
    /// let version = amx.find_pubvar::<f32>(c"my_plugin_version")?;
    ///
    /// if version.get() < 1.0 {
    ///     println!("You're badass");
    /// } else {
    ///     println!("Alright!");
    /// }
    /// #   Ok(())
    /// # }
    /// ```
    pub fn find_pubvar<T: Sized + AmxPrimitive>(&self, name: &CStr) -> AmxResult<Ref<'amx, T>> {
        let find_pubvar = FindPubVar::from_table(self.fn_table);
        let mut cell_ptr = 0;

        amx_try!(find_pubvar(self.ptr, name.as_ptr(), &mut cell_ptr));

        self.get_ref(cell_ptr)
    }

    /// Return flags of a compiled AMX.
    pub fn flags(&self) -> AmxResult<AmxFlags> {
        let flags = Flags::from_table(self.fn_table);
        let mut value: u16 = 0;

        amx_try!(flags(self.ptr, &mut value));

        Ok(AmxFlags::from_bits_truncate(value))
    }

    /// Get a handle ([`Ref<T>`]) to a value stored inside an AMX.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    /// # use samp_sdk::cell::Ref;
    /// # use samp_sdk::error::AmxResult;
    ///
    /// fn test_native(amx: Amx, cell_idx: i32) -> AmxResult<f32> {
    ///     let reference = amx.get_ref::<f32>(cell_idx)?;
    ///     return Ok(reference.get())
    /// }
    /// ```
    ///
    /// [`Ref<T>`]: ../cell/struct.Ref.html
    pub fn get_ref<T: Sized + AmxPrimitive>(&self, address: i32) -> AmxResult<Ref<'amx, T>> {
        let get_addr = GetAddr::from_table(self.fn_table);
        let mut dest = 0;
        let mut dest_addr = std::ptr::addr_of_mut!(dest);

        amx_try!(get_addr(self.ptr, address, &mut dest_addr));

        unsafe { Ok(Ref::new(address, dest_addr as *mut T)) }
    }

    #[inline(always)]
    pub(crate) fn release(&self, address: i32) {
        // Raw field access to avoid materializing `&mut AMX` (see
        // `exec_with_args`).
        unsafe {
            if (*self.ptr).hea > address {
                (*self.ptr).hea = address;
            }
        }
    }

    /// Push a value that implements [`AmxCell`] to an AMX stack.
    ///
    /// [`AmxCell`]: ../cell/repr/trait.AmxCell.html
    pub fn push<'a, T: AmxCell<'a>>(&'a self, value: T) -> AmxResult<()> {
        let push = Push::from_table(self.fn_table);

        amx_try!(push(self.ptr, value.as_cell()));

        Ok(())
    }

    /// Returns the length of a string in characters
    ///
    pub fn strlen(&self, value: *const i32) -> AmxResult<usize> {
        let strlen = StrLen::from_table(self.fn_table);
        let mut len = 0;
        amx_try!(strlen(value, &mut len));
        Ok(len as usize)
    }

    /// Get a heap [`Allocator`] for current [`Amx`].
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    ///
    /// # use samp_sdk::error::AmxResult;
    /// # use samp_sdk::consts::AmxExecIdx;
    /// #
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::null_mut(), 0) };
    /// let allocator = amx.allocator();
    /// let string = allocator.allot_string("Hello!")?;
    /// let player_id = 10;
    ///
    /// amx.push(string)?;
    /// amx.push(player_id)?;
    /// amx.exec(AmxExecIdx::UserDef(21))?;
    /// #
    /// #       Ok(())
    /// # }
    /// ```
    ///
    /// [`Allocator`]: struct.Allocator.html
    /// [`Amx`]: struct.Amx.html
    pub fn allocator(&self) -> Allocator<'amx> {
        Allocator::new(*self)
    }

    /// Returns a pointer to a raw [`AMX`] structure.
    ///
    /// [`AMX`]: ../raw/types/struct.AMX.html
    pub fn amx(&self) -> NonNull<AMX> {
        unsafe { NonNull::new_unchecked(self.ptr) }
    }

    /// Returns a pointer to an [`AMX_HEADER`].
    ///
    /// [`AMX_HEADER`]: ../raw/types/struct.AMX_HEADER.html
    pub fn header(&self) -> NonNull<AMX_HEADER> {
        unsafe { NonNull::new_unchecked((*self.ptr).base as *mut AMX_HEADER) }
    }
}

/// AMX memory allocator (on the heap) that frees captured memory after drop.
///
/// The handles it gives out ([`Ref`], [`Buffer`], [`AmxString`]) borrow the
/// *allocator*, because dropping it releases the heap frame they point into.
pub struct Allocator<'amx> {
    amx: Amx<'amx>,
    release_addr: i32,
}

impl<'amx> Allocator<'amx> {
    pub(crate) fn new(amx: Amx<'amx>) -> Allocator<'amx> {
        let release_addr = unsafe { (*amx.amx().as_ptr()).hea };

        Allocator { amx, release_addr }
    }

    /// The [`Amx`] this allocator allocates on.
    pub fn amx(&self) -> &Amx<'amx> {
        &self.amx
    }

    /// Allocate memory for a primitive value.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    ///
    /// # use samp_sdk::error::AmxResult;
    /// # use samp_sdk::consts::AmxExecIdx;
    /// #
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::null_mut(), 0) };
    /// // forward SomePublicFunc(player_id, &Float:health);
    /// let public_fn = amx.find_public(c"SomePublicFunc")?;
    /// let allocator = amx.allocator();
    ///
    /// let float_ref = allocator.allot(1.2f32)?;
    /// let player_id = 10;
    ///
    /// amx.push(float_ref)?;
    /// amx.push(player_id)?;
    /// amx.exec(public_fn)?;
    /// #
    /// #       Ok(())
    /// # }
    /// ```
    pub fn allot<T: Sized + AmxPrimitive>(&self, init_value: T) -> AmxResult<Ref<'_, T>> {
        let cell = self.amx.allot(1)?;
        cell.set(init_value);

        Ok(cell)
    }

    /// Allocate custom sized buffer on the heap.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    ///
    /// # use samp_sdk::error::AmxResult;
    /// # use samp_sdk::consts::AmxExecIdx;
    /// #
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::null_mut(), 0) };
    /// // forward SomePublicFunc(player_id, ids[], size);
    /// let public_fn = amx.find_public(c"SomePublicFunc")?;
    /// let allocator = amx.allocator();
    ///
    /// let size = 3;
    /// let buffer = allocator.allot_buffer(size)?;
    /// let player_id = 10;
    ///
    /// buffer.copy_from(&[5, 2, 15]);
    ///
    /// amx.push(size)?;
    /// amx.push(buffer)?;
    /// amx.push(player_id)?;
    /// amx.exec(public_fn)?;
    /// #
    /// #       Ok(())
    /// # }
    pub fn allot_buffer(&self, size: usize) -> AmxResult<Buffer<'_>> {
        let buffer = self.amx.allot(size)?;

        Ok(Buffer::new(buffer, size))
    }

    /// Allocate an array on the heap, copy values from the passed array and return `Buffer` containing reference to the allocated cell.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    ///
    /// # use samp_sdk::error::AmxResult;
    /// # use samp_sdk::consts::AmxExecIdx;
    /// #
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::null_mut(), 0) };
    /// // forward SomePublicFunc(player_id, ids[], size);
    /// let public_fn = amx.find_public(c"SomePublicFunc")?;
    /// let allocator = amx.allocator();
    ///
    /// let buffer = allocator.allot_array(&[5, 2, 15])?;
    /// let player_id = 10;
    ///
    /// amx.push(buffer.len())?;
    /// amx.push(buffer)?;
    /// amx.push(player_id)?;
    /// amx.exec(public_fn)?;
    /// #
    /// #       Ok(())
    /// # }
    pub fn allot_array<'alloc, T>(&'alloc self, array: &[T]) -> AmxResult<Buffer<'alloc>>
    where
        T: AmxCell<'alloc> + AmxPrimitive,
    {
        let buffer = self.allot_buffer(array.len())?;

        for (idx, item) in array.iter().enumerate() {
            buffer.set(idx, item.as_cell());
        }

        Ok(buffer)
    }

    /// Alocate a string, copy passed `&str` and return `AmxString` pointing to an `Amx` cell.
    pub fn allot_string(&self, string: &str) -> AmxResult<AmxString<'_>> {
        let bytes = Allocator::string_bytes(string);
        self.allot_bytes(bytes.as_ref())
    }

    /// Allocate raw bytes as a zero-terminated AMX string.
    ///
    /// Use this instead of [`allot_string`] when the bytes did not originate
    /// from UTF-8 — e.g. when passing through a string received from another
    /// script untouched.
    ///
    /// [`allot_string`]: #method.allot_string
    pub fn allot_bytes(&self, bytes: &[u8]) -> AmxResult<AmxString<'_>> {
        let buffer = self.allot_buffer(bytes.len() + 1)?;

        Ok(AmxString::new(buffer, bytes))
    }

    fn string_bytes(string: &str) -> Cow<'_, [u8]> {
        #[cfg(feature = "encoding")]
        return encoding::get().encode(string).0;

        #[cfg(not(feature = "encoding"))]
        return Cow::from(string.as_bytes());
    }
}

impl Drop for Allocator<'_> {
    fn drop(&mut self) {
        // AMX::release never fails
        self.amx.release(self.release_addr);
    }
}
