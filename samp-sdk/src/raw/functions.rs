use super::types::{AMX, AMX_NATIVE_INFO};
use std::ffi::c_void;
use std::os::raw::c_long;

pub type AmxNative = unsafe extern "C" fn(*mut AMX, params: *const i32) -> i32;
pub type AmxCallback =
    unsafe extern "C" fn(*mut AMX, index: i32, result: *mut i32, params: *const i32) -> i32;
pub type AmxDebug = unsafe extern "C" fn(*mut AMX) -> i32;

pub type Align16 = unsafe extern "C" fn(*mut u16) -> *mut u16;
pub type Align32 = unsafe extern "C" fn(*mut u32) -> *mut u32;
pub type Allot = unsafe extern "C" fn(*mut AMX, i32, *mut i32, *mut *mut i32) -> i32;
pub type Callback = unsafe extern "C" fn(*mut AMX, i32, *mut i32, *const i32) -> i32;
pub type Cleanup = unsafe extern "C" fn(*mut AMX) -> i32;
pub type Clone = unsafe extern "C" fn(*mut AMX, *mut AMX, *mut c_void) -> i32;
pub type Exec = unsafe extern "C" fn(*mut AMX, *mut i32, i32) -> i32;
pub type FindNative = unsafe extern "C" fn(*mut AMX, *const i8, *mut i32) -> i32;
pub type FindPublic = unsafe extern "C" fn(*mut AMX, *const i8, *mut i32) -> i32;
pub type FindPubVar = unsafe extern "C" fn(*mut AMX, *const i8, *mut i32) -> i32;
pub type FindTagId = unsafe extern "C" fn(*mut AMX, i32, *mut i8) -> i32;
pub type Flags = unsafe extern "C" fn(*mut AMX, *mut u16) -> i32;
pub type GetAddr = unsafe extern "C" fn(*mut AMX, i32, *mut *mut i32) -> i32;
pub type GetNative = unsafe extern "C" fn(*mut AMX, i32, *mut i8) -> i32;
pub type GetPublic = unsafe extern "C" fn(*mut AMX, i32, *mut i8) -> i32;
pub type GetPubVar = unsafe extern "C" fn(*mut AMX, i32, *mut i8, *mut i32) -> i32;
pub type GetString = unsafe extern "C" fn(*mut u8, *const i32, i32, usize) -> i32;
pub type GetTag = unsafe extern "C" fn(*mut AMX, i32, *mut i8, *mut i32) -> i32;
pub type GetUserData = unsafe extern "C" fn(*mut AMX, c_long, *mut *mut c_void) -> i32;
pub type Init = unsafe extern "C" fn(*mut AMX, *mut c_void) -> i32;
pub type InitJIT = unsafe extern "C" fn(*mut AMX, *mut c_void, *mut c_void) -> i32;
pub type MemInfo = unsafe extern "C" fn(*mut AMX, *mut c_long, *mut c_long, *mut c_long) -> i32;
pub type NameLength = unsafe extern "C" fn(*mut AMX, *mut i32) -> i32;
pub type NativeInfo = unsafe extern "C" fn(*const i8, AmxNative) -> *mut AMX_NATIVE_INFO;
pub type NumNatives = unsafe extern "C" fn(*mut AMX, *mut i32) -> i32;
pub type NumPublics = unsafe extern "C" fn(*mut AMX, *mut i32) -> i32;
pub type NumPubVars = unsafe extern "C" fn(*mut AMX, *mut i32) -> i32;
pub type NumTags = unsafe extern "C" fn(*mut AMX, *mut i32) -> i32;
pub type Push = unsafe extern "C" fn(*mut AMX, i32) -> i32;
pub type PushArray =
    unsafe extern "C" fn(*mut AMX, *mut i32, *mut *mut i32, *const i32, i32) -> i32;
pub type PushString =
    unsafe extern "C" fn(*mut AMX, *mut i32, *mut *mut i32, *const i8, i32, i32) -> i32;
pub type RaiseError = unsafe extern "C" fn(*mut AMX, i32) -> i32;
pub type Register = unsafe extern "C" fn(*mut AMX, *const AMX_NATIVE_INFO, i32) -> i32;
pub type Release = unsafe extern "C" fn(*mut AMX, i32) -> i32;
pub type SetCallback = unsafe extern "C" fn(*mut AMX, AmxCallback) -> i32;
pub type SetDebugHook = unsafe extern "C" fn(*mut AMX, AmxDebug) -> i32;
pub type SetString = unsafe extern "C" fn(*mut i32, *const i8, i32, i32, usize) -> i32;
pub type SetUserData = unsafe extern "C" fn(*mut AMX, c_long, *mut c_void) -> i32;
pub type StrLen = unsafe extern "C" fn(*const i32, *mut i32) -> i32;
pub type UTF8Check = unsafe extern "C" fn(*const i8, *mut i32) -> i32;
pub type UTF8Get = unsafe extern "C" fn(*const i8, *mut *const i8, *mut i32) -> i32;
pub type UTF8Len = unsafe extern "C" fn(*const i32, *mut i32) -> i32;
pub type UTF8Put = unsafe extern "C" fn(*mut i8, *mut *mut i8, i32, i32) -> i32;

pub type Logprintf = unsafe extern "C" fn(*const i8, ...);
