//! Core Amx types.
use crate::cell::{AmxString, Buffer, RawCell, Ref, ToAmxCell};
use crate::consts::{AmxExecIdx, AmxFlags};
use crate::error::{AmxError, AmxResult};
use crate::exports::*;
use crate::raw::types::{AMX, AMX_HEADER, AMX_NATIVE_INFO};

use std::cell::{Cell, RefCell};
use std::ffi::CStr;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

macro_rules! amx_try {
    ($call:expr) => {
        // SAFETY: `Amx` owns the loaded-VM invariant; each safe wrapper
        // validates its Rust inputs before invoking the corresponding SDK
        // function with pointers valid for the duration of this call.
        let result = unsafe { $call };

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
    ptr: NonNull<AMX>,
    fn_table: NonNull<usize>,
    brand: PhantomData<&'amx ()>,
}

impl<'amx> Amx<'amx> {
    /// Create an AMX wrapper from a raw pointer (passed by callbacks like
    /// `AmxLoad`) and the address of the server's `amx_*` exports table.
    ///
    /// # Safety
    /// `ptr` must identify an AMX that stays loaded for all of `'amx`, and
    /// `fn_table` must point to its complete SDK export table. Prefer branded
    /// entry points such as native parameters and `samp::amx::with`.
    pub unsafe fn new(ptr: NonNull<AMX>, fn_table: NonNull<usize>) -> Amx<'amx> {
        Amx {
            ptr,
            fn_table,
            brand: PhantomData,
        }
    }

    /// Register plugin native functions with this AMX.
    pub fn register(&self, natives: &[AMX_NATIVE_INFO]) -> AmxResult<()> {
        let register = unsafe { Register::from_table(self.fn_table) };
        let len = i32::try_from(natives.len()).map_err(|_| AmxError::Domain)?;
        let ptr = natives.as_ptr();

        amx_try!(register(self.ptr.as_ptr(), ptr, len));

        Ok(())
    }

    // `find_public`, `find_native` and `find_pubvar` take `&CStr`, not
    // `&str`: AMX symbol names are zero-terminated bytes, and accepting
    // UTF-8 here would invite lossy conversions from `AmxString` that
    // silently look up the wrong name. Use c"literals" or
    // `AmxString::to_cstring`.

    pub(crate) fn allot<T>(&self, cells: usize) -> AmxResult<Ref<'amx, T>> {
        let allot = unsafe { Allot::from_table(self.fn_table) };
        let cells = i32::try_from(cells).map_err(|_| AmxError::Domain)?;

        let mut amx_addr = 0;
        let mut phys_addr = std::ptr::null_mut();

        amx_try!(allot(
            self.ptr.as_ptr(),
            cells,
            &mut amx_addr,
            &mut phys_addr
        ));

        let storage = NonNull::new(phys_addr).ok_or(AmxError::MemoryAccess)?;
        unsafe { Ok(Ref::new(*self, amx_addr, storage)) }
    }

    /// Execute an AMX function.
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
    ///         Ok(money) => println!("Player {} has {} money.", 1, money.get()),
    ///         Err(err) => println!("Error: {:?}", err),
    ///     }
    /// }
    /// ```
    pub fn exec(&self, index: AmxExecIdx) -> AmxResult<RawCell> {
        let exec = unsafe { Exec::from_table(self.fn_table) };
        let mut retval = 0;

        amx_try!(exec(self.ptr.as_ptr(), &mut retval, index.into()));

        Ok(RawCell::new(retval))
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
    /// # let amx = unsafe { Amx::new(std::ptr::NonNull::dangling(), std::ptr::NonNull::dangling()) };
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
    ) -> AmxResult<RawCell> {
        // Raw field access: the AMX struct is owned by C code and other
        // handles to it exist, so no `&AMX`/`&mut AMX` is ever materialized.
        let ptr = self.ptr.as_ptr();
        let (saved_stk, saved_paramcount) = unsafe { ((*ptr).stk, (*ptr).paramcount) };
        let allocator = self.allocator();

        match push_args(&allocator) {
            Ok(()) => self.exec(index),
            Err(err) => {
                unsafe {
                    (*ptr).stk = saved_stk;
                    (*ptr).paramcount = saved_paramcount;
                }
                Err(err)
            }
        }
    }

    /// Return a native's index by name.
    ///
    /// # Examples
    /// See `find_public` and `exec` examples.
    pub fn find_native(&self, name: &CStr) -> AmxResult<i32> {
        let find_native = unsafe { FindNative::from_table(self.fn_table) };
        let mut index = -1;

        amx_try!(find_native(self.ptr.as_ptr(), name.as_ptr(), &mut index));

        Ok(index)
    }

    /// Return a public function's index by name.
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
        let find_public = unsafe { FindPublic::from_table(self.fn_table) };
        let mut index = -1;

        amx_try!(find_public(self.ptr.as_ptr(), name.as_ptr(), &mut index));

        Ok(AmxExecIdx::from(index))
    }

    /// Return a typed view of a public variable.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    /// # use samp_sdk::error::AmxResult;
    ///
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::NonNull::dangling(), std::ptr::NonNull::dangling()) };
    /// let version = amx.find_pubvar::<f32>(c"my_plugin_version")?;
    ///
    /// if version.get()? < 1.0 {
    ///     println!("You're badass");
    /// } else {
    ///     println!("Alright!");
    /// }
    /// #   Ok(())
    /// # }
    /// ```
    pub fn find_pubvar<T>(&self, name: &CStr) -> AmxResult<Ref<'amx, T>> {
        let find_pubvar = unsafe { FindPubVar::from_table(self.fn_table) };
        let mut cell_ptr = 0;

        amx_try!(find_pubvar(self.ptr.as_ptr(), name.as_ptr(), &mut cell_ptr));

        self.get_ref(cell_ptr)
    }

    /// Return flags of a compiled AMX.
    pub fn flags(&self) -> AmxResult<AmxFlags> {
        let flags = unsafe { Flags::from_table(self.fn_table) };
        let mut value: u16 = 0;

        amx_try!(flags(self.ptr.as_ptr(), &mut value));

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
    ///     reference.get()
    /// }
    /// ```
    ///
    /// [`Ref<T>`]: ../cell/struct.Ref.html
    pub fn get_ref<T>(&self, address: i32) -> AmxResult<Ref<'amx, T>> {
        let get_addr = unsafe { GetAddr::from_table(self.fn_table) };
        let mut dest = 0;
        let mut dest_addr = std::ptr::addr_of_mut!(dest);

        amx_try!(get_addr(self.ptr.as_ptr(), address, &mut dest_addr));

        let storage = NonNull::new(dest_addr).ok_or(AmxError::MemoryAccess)?;
        unsafe { Ok(Ref::new(*self, address, storage)) }
    }

    pub(crate) fn release(&self, address: i32) {
        let release = unsafe { Release::from_table(self.fn_table) };
        // A tracked heap frame only closes at the top of the AMX heap. The
        // SDK documents amx_Release as infallible for such an address.
        let _ = unsafe { release(self.ptr.as_ptr(), address) };
    }

    /// Push one cell-convertible value onto the AMX stack.
    ///
    pub fn push<T: ToAmxCell>(&self, value: T) -> AmxResult<()> {
        let push = unsafe { Push::from_table(self.fn_table) };

        amx_try!(push(self.ptr.as_ptr(), value.to_cell().get()));

        Ok(())
    }

    /// Return the length of an AMX string in characters.
    pub(crate) fn strlen(&self, value: *const i32) -> AmxResult<usize> {
        let strlen = unsafe { StrLen::from_table(self.fn_table) };
        let mut len = 0;
        amx_try!(strlen(value, &mut len));
        usize::try_from(len).map_err(|_| AmxError::Domain)
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
    /// # let amx = unsafe { Amx::new(std::ptr::NonNull::dangling(), std::ptr::NonNull::dangling()) };
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
        self.ptr
    }

    /// Returns a pointer to an [`AMX_HEADER`].
    ///
    /// [`AMX_HEADER`]: ../raw/types/struct.AMX_HEADER.html
    pub fn header(&self) -> NonNull<AMX_HEADER> {
        NonNull::new(unsafe { (*self.ptr.as_ptr()).base.cast() })
            .expect("a loaded AMX must have a header")
    }
}

/// AMX memory allocator (on the heap) that frees captured memory after drop.
///
/// The handles it gives out ([`Ref`], [`Buffer`], [`AmxString`]) borrow the
/// *allocator*, because dropping it releases the heap frame they point into.
/// Heap frames may be dropped out of order; release is deferred until every
/// newer frame is gone. While a nested frame exists, only that newest frame
/// may allocate.
pub struct Allocator<'amx> {
    amx: Amx<'amx>,
    frame: Rc<HeapFrame>,
}

struct HeapFrame {
    release_addr: i32,
    active: Cell<bool>,
}

struct AmxHeapFrames {
    amx: *mut AMX,
    frames: Vec<Rc<HeapFrame>>,
}

thread_local! {
    static HEAP_FRAMES: RefCell<Vec<AmxHeapFrames>> = const { RefCell::new(Vec::new()) };
}

fn open_heap_frame(amx: *mut AMX, release_addr: i32) -> Rc<HeapFrame> {
    let frame = Rc::new(HeapFrame {
        release_addr,
        active: Cell::new(true),
    });
    HEAP_FRAMES.with_borrow_mut(|all| {
        if let Some(heap) = all.iter_mut().find(|heap| heap.amx == amx) {
            heap.frames.push(Rc::clone(&frame));
        } else {
            all.push(AmxHeapFrames {
                amx,
                frames: vec![Rc::clone(&frame)],
            });
        }
    });
    frame
}

fn heap_frame_is_top(amx: *mut AMX, frame: &Rc<HeapFrame>) -> bool {
    HEAP_FRAMES.with_borrow(|all| {
        all.iter()
            .find(|heap| heap.amx == amx)
            .and_then(|heap| heap.frames.last())
            .is_some_and(|top| Rc::ptr_eq(top, frame))
    })
}

fn close_heap_frame(amx: *mut AMX, frame: &Rc<HeapFrame>) -> Option<i32> {
    frame.active.set(false);
    HEAP_FRAMES.with_borrow_mut(|all| {
        let heap_index = all
            .iter()
            .position(|heap| heap.amx == amx)
            .expect("an allocator must have a tracked heap frame");
        let heap = &mut all[heap_index];
        debug_assert!(heap.frames.iter().any(|item| Rc::ptr_eq(item, frame)));

        let mut release_addr = None;
        while heap.frames.last().is_some_and(|top| !top.active.get()) {
            release_addr = heap.frames.pop().map(|closed| closed.release_addr);
        }
        if heap.frames.is_empty() {
            all.swap_remove(heap_index);
        }
        release_addr
    })
}

impl<'amx> Allocator<'amx> {
    pub(crate) fn new(amx: Amx<'amx>) -> Allocator<'amx> {
        let release_addr = unsafe { (*amx.amx().as_ptr()).hea };
        let frame = open_heap_frame(amx.amx().as_ptr(), release_addr);

        Allocator { amx, frame }
    }

    fn allot_cells<T>(&self, cells: usize) -> AmxResult<Ref<'_, T>> {
        if !heap_frame_is_top(self.amx.amx().as_ptr(), &self.frame) {
            return Err(AmxError::InvalidState);
        }
        self.amx.allot(cells)
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
    /// # let amx = unsafe { Amx::new(std::ptr::NonNull::dangling(), std::ptr::NonNull::dangling()) };
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
    pub fn allot<T: ToAmxCell>(&self, init_value: T) -> AmxResult<Ref<'_, T>> {
        let cell = self.allot_cells(1)?;
        cell.set(init_value);

        Ok(cell)
    }

    /// Allocate a buffer of `size` cells on the AMX heap.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    ///
    /// # use samp_sdk::error::AmxResult;
    /// # use samp_sdk::consts::AmxExecIdx;
    /// #
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::NonNull::dangling(), std::ptr::NonNull::dangling()) };
    /// // forward SomePublicFunc(player_id, ids[], size);
    /// let public_fn = amx.find_public(c"SomePublicFunc")?;
    /// let allocator = amx.allocator();
    ///
    /// let size = 3_i32;
    /// let buffer = allocator.allot_buffer(size as usize)?;
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
        let buffer = self.allot_cells(size)?;

        Ok(Buffer::new(buffer, size))
    }

    /// Copy a Rust slice into a newly allocated AMX buffer.
    ///
    /// # Example
    /// ```rust,no_run
    /// use samp_sdk::amx::Amx;
    ///
    /// # use samp_sdk::error::AmxResult;
    /// # use samp_sdk::consts::AmxExecIdx;
    /// #
    /// # fn main() -> AmxResult<()> {
    /// # let amx = unsafe { Amx::new(std::ptr::NonNull::dangling(), std::ptr::NonNull::dangling()) };
    /// // forward SomePublicFunc(player_id, ids[], size);
    /// let public_fn = amx.find_public(c"SomePublicFunc")?;
    /// let allocator = amx.allocator();
    ///
    /// let buffer = allocator.allot_array(&[5, 2, 15])?;
    /// let player_id = 10;
    ///
    /// let length = i32::try_from(buffer.len())
    ///     .map_err(|_| samp_sdk::error::AmxError::Domain)?;
    /// amx.push(length)?;
    /// amx.push(buffer)?;
    /// amx.push(player_id)?;
    /// amx.exec(public_fn)?;
    /// #
    /// #       Ok(())
    /// # }
    pub fn allot_array<'alloc, T: ToAmxCell>(
        &'alloc self,
        array: &[T],
    ) -> AmxResult<Buffer<'alloc>> {
        let buffer = self.allot_buffer(array.len())?;
        buffer.copy_from(array);

        Ok(buffer)
    }

    /// Allocate a zero-terminated AMX string from UTF-8 text.
    pub fn allot_string(&self, string: &str) -> AmxResult<AmxString<'_>> {
        self.allot_bytes(string.as_bytes())
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
}

impl Drop for Allocator<'_> {
    fn drop(&mut self) {
        if let Some(release_addr) = close_heap_frame(self.amx.amx().as_ptr(), &self.frame) {
            self.amx.release(release_addr);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use super::{close_heap_frame, heap_frame_is_top, open_heap_frame, AMX};

    #[test]
    fn out_of_order_heap_frames_defer_release() {
        let amx = NonNull::<AMX>::dangling().as_ptr();
        let outer = open_heap_frame(amx, 0);
        let inner = open_heap_frame(amx, 4);

        assert!(!heap_frame_is_top(amx, &outer));
        assert!(heap_frame_is_top(amx, &inner));
        assert_eq!(close_heap_frame(amx, &outer), None);
        assert_eq!(close_heap_frame(amx, &inner), Some(0));
    }

    #[test]
    fn nested_heap_frames_release_independently() {
        let amx = NonNull::<AMX>::dangling().as_ptr();
        let outer = open_heap_frame(amx, 0);
        let inner = open_heap_frame(amx, 4);

        assert_eq!(close_heap_frame(amx, &inner), Some(4));
        assert_eq!(close_heap_frame(amx, &outer), Some(0));
    }
}
