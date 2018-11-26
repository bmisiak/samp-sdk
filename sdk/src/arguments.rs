use crate::prelude::*;

pub trait AmxValueDecode where Self: Sized {
    fn decode(_amx: &Amx, _value: usize, _size: Option<usize>) -> AmxResult<Self>;
}

// pub trait AmxValueEncode where Self: Sized {
//     fn encode(self) -> AmxResult<usize>;
// }

macro_rules! impl_primitives {
    ($type:ty) => {
        impl AmxValueDecode for $type {
            fn decode(_amx: &Amx, value: usize, _size: Option<usize>) -> AmxResult<Self> {
                unsafe {
                    Ok(std::mem::transmute(value))
                }
            }
        }

        impl AmxValueDecode for &$type {
            fn decode(amx: &Amx, value: usize, _size: Option<usize>) -> AmxResult<Self> {
                amx.get_address(value).map(|item| item as &$type)
            }
        }

        impl AmxValueDecode for &mut $type {
            fn decode(amx: &Amx, value: usize, _size: Option<usize>) -> AmxResult<Self> {
                amx.get_address(value)
            }
        }
    };
}

impl_primitives!(i32);
impl_primitives!(u32);
impl_primitives!(f32);
impl_primitives!(usize);
impl_primitives!(isize);

impl AmxValueDecode for String {
    fn decode(amx: &Amx, value: usize, _size: Option<usize>) -> AmxResult<Self> {
        amx.get_address(value)
            .and_then(|ptr| {
                let length = amx.string_len(ptr)?;
                amx.get_string(ptr, length + 1)
            })
    }
}