#![forbid(unsafe_code)]

fn add(left: i32, right: i32) -> i32 {
    left + right
}

fn process_tick() {}

samp::plugin! {
    natives: [c"Add" = add],
    process_tick: process_tick,
    load: {}
}

#[test]
fn process_tick_support_is_advertised() {
    assert_ne!(Supports() & samp::consts::Supports::PROCESS_TICK.bits(), 0);
}
