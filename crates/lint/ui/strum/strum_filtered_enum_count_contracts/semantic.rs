#![allow(clippy::missing_const_for_fn, dead_code, unknown_lints)]

use strum::EnumCount;

#[derive(strum::EnumCount)]
enum Capability {
    Read,
    Write,
    Unknown,
}

fn supported_count() -> usize {
    Capability::COUNT
}

fn main() {}
