/*! 

# SA:MP SDK
This crate is a Rust language wrapper for SA:MP SDK.

*/

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]
#![allow(non_snake_case)]

#[macro_use] pub mod macros;
pub mod args;
pub mod consts;
pub mod data;
pub mod types;
pub mod amx;
pub mod cp1251;

pub use lazy_static::{lazy_static, __lazy_static_internal, __lazy_static_create};

pub mod prelude {
    pub use crate::amx::{AMX, AmxResult, AmxError};
    pub use crate::types::Cell;
}