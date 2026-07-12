# samp-rs

Rust bindings and safe wrappers for writing SA-MP plugins.

The workspace contains:

- `samp`: plugin lifecycle, AMX handles, logging, and public macros;
- `samp-sdk`: raw SDK bindings plus safe cells, arguments, references, buffers,
  strings, and allocation.

SA-MP itself is 32-bit; plugin builds target an i686 platform.

## Example

```rust,no_run
use samp::prelude::*;
fn test_native(_amx: Amx, text: AmxString) -> AmxResult<bool> {
    println!("{}", text.to_string_lossy());
    Ok(true)
}

samp::plugin! {
    natives: [c"TestNative" = test_native],
    load: {
        println!("Plugin loaded");
    }
}
```

Native arguments are decoded before the Rust function is called. Checked
types such as `u32`, `usize`, and `NonZeroUsize` reject cells outside their
domain. Raw bit patterns remain available explicitly through `RawCell`. A
final `VariadicArgs` parameter consumes a PAWN variadic tail.
