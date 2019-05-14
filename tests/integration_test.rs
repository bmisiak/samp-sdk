#[macro_use] extern crate samp_sdk;
use samp_sdk::types::Cell;
use std::ffi::{CString};
use samp_sdk::consts::*;
use samp_sdk::amx::{AMX, AmxResult};

pub struct MyPlugin;
impl Default for MyPlugin {
    fn default() -> Self { MyPlugin {} }
}

new_plugin!(MyPlugin);

impl MyPlugin {
    pub fn load(&self) -> bool { true }
    pub fn unload(&self) {  }
    pub fn amx_load(&mut self, _amx: &AMX) -> Cell { AMX_ERR_NONE }
    pub fn amx_unload(&self, _amx: &AMX) -> Cell { AMX_ERR_NONE }

    pub fn is_string_long(&self, _amx: &AMX, string: CString) -> AmxResult<Cell> {
        if string.to_bytes().len() > 48 {
            Ok(1)
        } else {
            Ok(0)
        }
    }
}

define_native!(is_string_long, string: CString);

#[test]
fn test() -> Result<(),samp_sdk::amx::AmxError> {
    unsafe {
        let amx: samp_sdk::amx::AMX = std::mem::uninitialized(); 
        let _cell: *mut Cell = std::mem::uninitialized();

        let playerid = 1 as i32;
        let new_name = CString::new("Name_Surname").unwrap();
        
        let _result = exec_native!(amx, "SetPlayerName"; playerid, &new_name => string)?;

        Ok(())
        //amx.native("SetPlayerName").int(playerid).string(&new_name).exec();
        //log!("is str long: {}", is_string_long(amx, cell));
    }
}