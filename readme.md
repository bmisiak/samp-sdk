[![Docs](https://docs.rs/samp/badge.svg)](https://docs.rs/samp)
[![Crates](https://img.shields.io/crates/v/samp.svg)](https://crates.io/crates/samp)
# samp-rs
samp-rs is a tool to develop plugins for [samp](http://sa-mp.com) servers written in rust.

# documentation
it's [here](https://zottce.github.io/samp-rs/samp/index.html)! need to find a way to fix docs.rs ...

# project structure
* `samp` is a glue between crates described below (that's what you need).
* `samp-codegen` generates raw `extern "C"` functions and does whole nasty job.
* `samp-sdk` contains all types to work with amx.

# usage
* [install](https://rustup.rs) rust compiler (supports only `i686` os versions because of samp server arch).
* add in your `Cargo.toml` this:
```toml
[lib]
crate-type = ["cdylib"] # or dylib

[dependencies]
samp = "0.1.2"
```
* write your first plugin

<<<<<<< HEAD
## Features
Hides most of type coercion. You don't need make a `cell` type as a `CString` or other things yourself.

Macros:
* `new_plugin!` that defines a plugin and exports functions.
* `define_native!` defines a native and parses arguments.
* `log!` calls `logprinft` funciton.
* `natives!` makes a vec of your natives.
* `get_array!` converts pointer to a `slice`
=======
# migration from old versions
* check out [the guide](migration.md)

# examples
* simple memcache plugin in `plugin-example` folder.
* your `lib.rs` file
```rust
use samp::prelude::*; // export most useful types
use samp::{native, initialize_plugin}; // codegen macros
>>>>>>> 659646d91200e932fe804935b8cf48b0d6bb8dd7

struct Plugin;

impl SampPlugin for Plugin {
    // this function executed when samp server loads your plugin
    fn on_load(&mut self) {
        println!("Plugin is loaded.");
    }
}

impl Plugin {
    #[native(name = "TestNative")]
    fn my_native(&mut self, _amx: &Amx, text: AmxString) -> AmxResult<bool> {
        let text = text.to_string(); // convert amx string into rust string
        println!("rust plugin: {}", text);

        Ok(true)
    }
}

initialize_plugin!(
    natives: [Plugin::my_native],
    {
        let plugin = Plugin; // create a plugin object
        return plugin; // return the plugin into runtime
    }
<<<<<<< HEAD
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
=======
)
```
>>>>>>> 659646d91200e932fe804935b8cf48b0d6bb8dd7
