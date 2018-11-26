use samp_sdk::prelude::*;

#[derive(Default, SampPlugin)]
pub struct Plugin;

#[contains_natives]
impl Plugin {
    fn load(&mut self) {}

    fn unload(&mut self) {}

    fn amx_load(&mut self, _amx: &Amx) {}

    fn amx_unload(&mut self, _amx: &Amx) {}

    #[native]
    fn my_native(&mut self, _amx: &Amx, _value: u32, _ptr: &mut u32, _str: String) -> AmxResult<u32> {
        return Ok(1);
    }
}
