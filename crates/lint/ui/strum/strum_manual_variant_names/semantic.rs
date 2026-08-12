#![allow(dead_code, unknown_lints)]

enum Stage {
    Planned,
    Complete,
}

impl Stage {
    const NAMES: [&'static str; 2] = ["Planned", "Complete"];
}

fn main() {}
