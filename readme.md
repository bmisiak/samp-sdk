# SA:MP SDK
[![Build Status](https://travis-ci.com/ZOTTCE/samp-sdk.svg?branch=master)](https://travis-ci.com/ZOTTCE/samp-sdk)
[![Build status](https://ci.appveyor.com/api/projects/status/ldq6suvwlmka072o/branch/master?svg=true)](https://ci.appveyor.com/project/ZOTTCE/samp-sdk/branch/master)
[![Docs](https://docs.rs/samp-sdk/badge.svg)](https://docs.rs/samp-sdk)
[![Crates](https://img.shields.io/crates/v/samp-sdk.svg)](https://crates.io/crates/samp-sdk)

Pretty cool and beautiful bindings for SA:MP SDK. Read [Get Started](https://github.com/ZOTTCE/samp-sdk/wiki/Get-started) wiki page.

## Features
Hides most of type coercion. You don't need make a `cell` type as a `CString` or other things yourself.

Macros:
* `new_plugin!` that defines a plugin and exports functions.
* `define_native!` defines a native and parses arguments.
* `log!` calls `logprinft` funciton.
* `natives!` makes a vec of your natives.
* `get_array!` converts pointer to a `slice`

### Useful macros
#### Make a new plugin
``` Rust
define_native!(my_function, playerid: i32);

struct Plugin {
    version: &'static str,
    amx_count: u32,
}

impl Plugin {
    fn load(&self) -> bool {
        log!("Plugin is loaded. Version: {}", self.version);
        return true;
    }

    fn unload(&self) {
       log!("Plugin has unloaded");
    }

    fn amx_load(&mut self, amx: &AMX) -> Cell {
        let natives = natives![
            { "MyFunction", my_function }
        ];

        match amx.register(&natives) {
            Ok(_) => log!("Natives are successful loaded"),
            Err(err) => log!("Whoops, there is an error {:?}", err),
        }

        self.amx_count += 1;

        AMX_ERR_NONE
    }

    fn amx_unload(&mut self, _: &AMX) -> Cell {
        self.amx_count -= 1;

        AMX_ERR_NONE
    }

    fn my_function(&self, _amx: &AMX, player_id: i32) -> AmxResult<Cell> {
        Ok(-player_id)
    }
}

impl Default for Plugin {
    fn default() -> Self {
        Plugin {
            version: "0.1",
            amx_count: 0,
        }
    }
}

new_plugin!(Plugin);

// Also you can make a plugin with ProcessTick support.
new_plugin!(Plugin with process_tick)
```
#### Define a native function.
Hides arguments parsing inside the macro.

All you need are to define a method `function_name` in your new plugin with given arguments.
``` Rust
// native: FunctionName(int_arg, &float_arg);
define_native!(function_name, int_arg: i32, float_ref_arg: ref f32);

// native: WithoutArguments();
define_native(function_name);
```

#### Call natives and public functions.
``` Rust
// Broadcast to all subscribers that a user have changed his name.
fn notify(&self, amx: AMX, player_id: u32, old_name: CString, new_name: CString) -> AmxResult<Cell> {
    exec_public!(amx, "OnPlayerNameChanged"; player_id, old_name => string, new_name => string)
}
```

## TODO List
* Develop a new samp-plugin-example that shows all good points of this samp-sdk.

## Documentation
[Here](https://docs.rs/samp-sdk).

## Plugin example
[Here](https://github.com/ZOTTCE/samp-plugin-example) you can see such a beautiful example of the samp-sdk.
