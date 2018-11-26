pub use samp_sdk_codegen::contains_natives;
pub use samp_sdk_codegen::SampPlugin;

pub mod amx;
pub mod consts;
pub mod error;
pub mod internal;
pub mod types;
pub mod arguments;
pub mod cp1251;

pub trait SampPlugin {
    type Plugin;

    fn get() -> &'static mut Self::Plugin;
    fn internal() -> &'static mut crate::internal::InternalData;
}

pub mod prelude {
    pub use crate::contains_natives;
    pub use crate::error::{AmxError, AmxResult};
    pub use crate::types::{Cell, Ucell};
    pub use crate::SampPlugin;
    pub use crate::amx::Amx;
}
