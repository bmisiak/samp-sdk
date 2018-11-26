/*!
    List of AMX types such as `Cell`, `AmxNativeInfo` and signatures of functions.
*/
use std::os::raw::c_void;

pub type Cell = i32;
pub type Ucell = u32;

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct RawAmx {
    pub base: *mut u8,
    pub data: *mut u8,
    pub callback: AmxCallback,
    pub debug: AmxDebug,
    pub cip: Cell,
    pub frm: Cell,
    pub hea: usize,
    pub hlw: Cell,
    pub stk: usize,
    pub stp: usize,
    pub flags: i32,
    pub usertags: [i64; 4usize],
    pub userdata: [*mut c_void; 4usize],
    pub error: i32,
    pub paramcount: i32,
    pub pri: Cell,
    pub alt: Cell,
    pub reset_stk: Cell,
    pub reset_hea: Cell,
    pub sysreq_d: Cell,
}

pub type AmxNative = extern "C" fn(*mut RawAmx, params: *mut Cell) -> Cell;
pub type AmxCallback =
    extern "C" fn(*mut RawAmx, index: Cell, result: *mut Cell, params: *mut Cell) -> i32;
pub type AmxDebug = extern "C" fn(*mut RawAmx) -> i32;

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct AmxNativeInfo {
    pub name: *const i8,
    pub func: AmxNative,
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct AmxFuncstub {
    pub address: Ucell,
    pub name: [i8; 20usize],
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct FuncstubNt {
    pub address: Ucell,
    pub nameofs: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct AmxHeader {
    pub size: usize,
    pub magic: u16,
    pub file_version: i8,
    pub amx_version: i8,
    pub flags: i16,
    pub defsize: i16,
    pub cod: usize,
    pub dat: usize,
    pub hea: usize,
    pub stp: usize,
    pub cip: usize,
    pub publics: usize,
    pub natives: usize,
    pub libraries: usize,
    pub pubvars: usize,
    pub tags: usize,
    pub nametable: usize,
}

pub type Align16 = extern "C" fn(*mut u16) -> *mut u16;
pub type Align32 = extern "C" fn(*mut u32) -> *mut u32;
pub type Allot = extern "C" fn(*mut RawAmx, i32, *mut Cell, *mut Cell) -> i32;
pub type Callback = extern "C" fn(*mut RawAmx, Cell, *mut Cell, *mut Cell) -> i32;
pub type Cleanup = extern "C" fn(*mut RawAmx) -> i32;
pub type Clone = extern "C" fn(*mut RawAmx, *mut RawAmx, *mut c_void) -> i32;
pub type Exec = extern "C" fn(*mut RawAmx, *mut Cell, i32) -> i32;
pub type FindNative = extern "C" fn(*mut RawAmx, *const i8, *mut i32) -> i32;
pub type FindPublic = extern "C" fn(*mut RawAmx, *const i8, *mut i32) -> i32;
pub type FindPubVar = extern "C" fn(*mut RawAmx, *const i8, *mut Cell) -> i32;
pub type FindTagId = extern "C" fn(*mut RawAmx, Cell, *mut i8) -> i32;
pub type Flags = extern "C" fn(*mut RawAmx, *mut u16) -> i32;
pub type GetAddr = extern "C" fn(*mut RawAmx, Cell, *mut *mut Cell) -> i32;
pub type GetNative = extern "C" fn(*mut RawAmx, i32, *mut i8) -> i32;
pub type GetPublic = extern "C" fn(*mut RawAmx, i32, *mut i8) -> i32;
pub type GetPubVar = extern "C" fn(*mut RawAmx, i32, *mut i8, *mut Cell) -> i32;
pub type GetString = extern "C" fn(*mut u8, *const Cell, i32, usize) -> i32;
pub type GetTag = extern "C" fn(*mut RawAmx, i32, *mut i8, *mut Cell) -> i32;
pub type GetUserData = extern "C" fn(*mut RawAmx, i64, *mut *mut c_void) -> i32;
pub type Init = extern "C" fn(*mut RawAmx, *mut c_void) -> i32;
pub type InitJIT = extern "C" fn(*mut RawAmx, *mut c_void, *mut c_void) -> i32;
pub type MemInfo = extern "C" fn(*mut RawAmx, *mut i32, *mut i32, *mut i32) -> i32;
pub type NameLength = extern "C" fn(*mut RawAmx, *mut i32) -> i32;
pub type NativeInfo = extern "C" fn(*const i8, AmxNative) -> *mut AmxNativeInfo;
pub type NumNatives = extern "C" fn(*mut RawAmx, *mut i32) -> i32;
pub type NumPublics = extern "C" fn(*mut RawAmx, *mut i32) -> i32;
pub type NumPubVars = extern "C" fn(*mut RawAmx, *mut i32) -> i32;
pub type NumTags = extern "C" fn(*mut RawAmx, *mut i32) -> i32;
pub type Push = extern "C" fn(*mut RawAmx, Cell) -> i32;
pub type PushArray = extern "C" fn(*mut RawAmx, *mut Cell, *mut *mut Cell, *const Cell, i32) -> i32;
pub type PushString =
    extern "C" fn(*mut RawAmx, *mut Cell, *mut *mut Cell, *const i8, i32, i32) -> i32;
pub type RaiseError = extern "C" fn(*mut RawAmx, i32) -> i32;
pub type Register = extern "C" fn(*mut RawAmx, *const AmxNativeInfo, i32) -> i32;
pub type Release = extern "C" fn(*mut RawAmx, Cell) -> i32;
pub type SetCallback = extern "C" fn(*mut RawAmx, AmxCallback) -> i32;
pub type SetDebugHook = extern "C" fn(*mut RawAmx, AmxDebug) -> i32;
pub type SetString = extern "C" fn(*mut Cell, *const i8, i32, i32, usize) -> i32;
pub type SetUserData = extern "C" fn(*mut RawAmx, i64, *mut c_void) -> i32;
pub type StrLen = extern "C" fn(*const Cell, *mut i32) -> i32;
pub type UTF8Check = extern "C" fn(*const i8, *mut i32) -> i32;
pub type UTF8Get = extern "C" fn(*const i8, *mut *const i8, *mut Cell) -> i32;
pub type UTF8Len = extern "C" fn(*const Cell, *mut i32) -> i32;
pub type UTF8Put = extern "C" fn(*mut i8, *mut *mut i8, i32, Cell) -> i32;

pub type Logprintf = extern "C" fn(*const i8, ...);
