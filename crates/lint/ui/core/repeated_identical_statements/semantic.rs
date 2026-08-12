#![warn(repeated_identical_statements)]
#![allow(dead_code, misordered_module_declarations)]

fn record(_: &str) {}

fn accidental() {
    record("ready");
    record("ready");
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
