use crate::internal::InternalData;
use crate::types::*;
use crate::error::{AmxResult, AmxError};
use crate::cp1251;
use crate::consts::Exports;

use std::mem::{transmute, transmute_copy, size_of};

/// Converts a raw AMX error to `AmxError`.
macro_rules! ret {
    ($res:ident, $ret:expr) => {
        {
            if $res == 0 {
                Ok($ret)
            } else {
                Err(AmxError::from($res))
            }
        }
    }
}

/// Makes an call to any AMX functions and uses `ret!`.
macro_rules! call {
    (
        $ex:expr
        =>
        $ret:expr
    ) => {
        {
            let result = $ex;
            ret!(result, $ret)
        }
    }
}

/// Gets a function from a raw pointer in `data::amx_functions`.
macro_rules! import {
    ($amx:expr, $type:ident) => {
        unsafe {
            let func_ptr = $amx.internal.amx_function(Exports::$type as isize) as *const $crate::types::$type;
            func_ptr.read()
        }
    };
}

pub struct Amx {
    pub raw_ptr: *mut RawAmx,
    pub internal: &'static mut InternalData,
}

impl Amx {
    pub fn allot(&self, cells: usize) -> AmxResult<(usize, usize)> {
        let amx_addr = 0;
        let phys_addr = 0;

        let allot = import!(self, Allot);

        unsafe {
            call!(allot(self.raw_ptr, cells as i32, transmute(&amx_addr), transmute(&phys_addr)) => (amx_addr, phys_addr))
        }
    }

    /// Frees all memory **above** input address.
    pub fn release(&self, address: usize) -> AmxResult<()> {
        let release = import!(self, Release);
        call!(release(self.raw_ptr, address as i32) => ())
    }

    pub fn flags(&self) -> AmxResult<u16> {
        let flags = import!(self, Flags);
        let value: u16 = 0;

        unsafe {
            call!(flags(self.raw_ptr, transmute(&value)) => value)
        }
    }

    pub fn mem_info(&self) -> AmxResult<(i64, i64, i64)> {
        let mem_info = import!(self, MemInfo);
        let codesize: i64 = 0;
        let datasize: i64 = 0;
        let stackheap: i64 = 0;

        unsafe {
            call!(mem_info(self.raw_ptr, transmute(&codesize), transmute(&datasize), transmute(&stackheap)) => (codesize, datasize, stackheap))
        }
    }

    pub fn push<T: Sized>(&self, value: T) -> AmxResult<()> {
        let push = import!(self, Push);

        unsafe {
            call!(push(self.raw_ptr, transmute_copy(&value)) => ())
        }
    }

    pub fn push_array<T: Sized>(&self, array: &[T]) -> AmxResult<usize> {
        let (amx_addr, phys_addr) = self.allot(array.len())?;
        let dest = phys_addr as *mut usize;

        for i in 0..array.len() {
            unsafe {
                *(dest.offset(i as isize)) = transmute_copy(&array[i]);
            }
        }

        self.push(amx_addr)?;
        Ok(amx_addr)
    }

    pub fn push_string(&self, string: &str, packed: bool) -> AmxResult<usize> {
        if packed {
            unimplemented!()
        } else {
            let bytes = cp1251::encode(string)?;
            let (amx_addr, phys_addr) = self.allot(bytes.len() + 1)?;
            let dest = phys_addr as *mut usize;

            for i in 0..string.len() {
                unsafe {
                    *(dest.offset(i as isize)) = transmute_copy(&bytes[i]);
                }
            }

            unsafe {
                *(dest.offset(string.len() as isize)) = 0;
            }

            self.push(amx_addr)?;
            Ok(amx_addr)
        }
    }

    pub fn exec(&self, index: i32) -> AmxResult<i32> {
        let exec = import!(self, Exec);

        let retval = -1;
        unsafe {
            call!(exec(self.raw_amx, transmute(&retval), index) => retval)
        }
    }

    pub fn get_address<'a, T: Sized>(&self, address: usize) -> AmxResult<&'a mut T> {
        unsafe {
            let data = self.data_section();

            if address >= (*self.raw_ptr).hea && address < (*self.raw_ptr).stk || address >= (*self.raw_ptr).stp {
                Err(AmxError::MemoryAccess)
            } else {
                Ok(transmute(data + address as usize))
            }
        }
    }

    /// Gets length of a string.
    pub fn string_len(&self, address: *const Cell) -> AmxResult<usize> {
        let string_len = import!(self, StrLen);
        let mut length = 0;

        call!(string_len(address, &mut length) => length as usize)
    }

    pub fn get_string(&self, address: *const Cell, size: usize) -> AmxResult<String> {
        const UNPACKEDMAX: u32 = ((1u32 << (size_of::<u32>() - 1) * 8) - 1u32);
        const CHARBITS: usize = 8 * size_of::<u8>();

        let mut string = Vec::with_capacity(size);

        unsafe {
            if address.read() as u32 > UNPACKEDMAX {
                // packed string
                let mut i = size_of::<Cell>() - 1;
                let mut cell = 0;
                let mut ch;
                let mut length = 0;
                let mut offset = 0;

                while length < size {
                    if i == size_of::<Cell>() - 1 {
                        cell = address.offset(offset).read();
                        offset += 1;
                    }

                    ch = (cell >> i * CHARBITS) as u8;

                    if ch == 0 {
                        break;
                    }

                    string.push(ch);
                    length += 1;
                    i = (i + size_of::<Cell>() - 1) % size_of::<Cell>();
                }
            } else {
                let mut length = 0;
                let mut byte = address.offset(length).read();

                while byte != 0 && length < size as isize {
                    string.push(byte as u8);
                    length += 1;
                    byte = address.offset(length).read();
                }
            }

            cp1251::decode(string.as_slice())
        }
    }

    /// Raises an AMX error.
    pub fn raise_error(&self, error: AmxError) -> AmxResult<()> {
        let raise_error = import!(self, RaiseError);
        call!(raise_error(self.raw_ptr, error as i32) => ())
    }

    #[inline(always)]
    pub fn header(&self) -> &AmxHeader {
        unsafe {
            &*((*self.raw_ptr).base as *const AmxHeader)
        }
    }

    #[inline(always)]
    pub fn header_mut(&self) -> &mut AmxHeader {
        unsafe {
            &mut *((*self.raw_ptr).base as *mut AmxHeader)
        }
    }

    #[inline]
    pub fn data_section(&self) -> usize {
        unsafe {
            if (*self.raw_ptr).data.is_null() {
                (*self.raw_ptr).base as usize + self.header().dat as usize
            } else {
                (*self.raw_ptr).data as usize
            }
        }
    }
}