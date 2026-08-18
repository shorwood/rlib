#![warn(rlib::repeated_identical_statements)]
#![allow(dead_code, rlib::misordered_module_declarations)]

fn record(_: &str) {}

fn accidental() {
    record("ready");
    record("ready");
}

fn explained_repetition() {
    record("retry");
    // The repeated operation is deliberate because the receiver requires two pulses.
    record("retry");
}

fn explicit_repetition() {
    for _ in 0..2 {
        record("ready");
    }
}

macro_rules! generated_repetition {
    () => {
        record("generated");
        record("generated");
    };
}

fn main() {
    generated_repetition!();
}
