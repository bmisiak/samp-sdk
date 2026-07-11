# samp-rs

Rust bindings and safe wrappers for writing SA-MP plugins.

The workspace contains:

- `samp`: plugin lifecycle, AMX handles, logging, and public macros;
- `samp-codegen`: `#[native]` and `initialize_plugin!`;
- `samp-sdk`: raw SDK bindings plus safe cells, arguments, references, buffers,
  strings, and allocation.

SA-MP itself is 32-bit; plugin builds target an i686 platform.

## Example

```rust,no_run
use samp::prelude::*;
use samp::{initialize_plugin, native};

#[native(name = "TestNative")]
fn test_native(_amx: Amx, text: AmxString) -> AmxResult<bool> {
    println!("{}", text.to_string_lossy());
    Ok(true)
}

initialize_plugin!(
    natives: [test_native],
    {
        println!("Plugin loaded");
    }
);
```

Native arguments are decoded before the Rust function is called. Checked
types such as `u32`, `usize`, and `NonZeroUsize` reject cells outside their
domain. Raw bit patterns remain available explicitly through `RawCell`.

See [migration.md](migration.md) for the current breaking API changes.
