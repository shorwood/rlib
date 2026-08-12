#![allow(dead_code, unknown_lints)]

#[derive(strum::Display, strum::EnumString)]
#[strum(prefix = "state-")]
enum State {
    Pending,
    Complete,
}

fn main() {}
