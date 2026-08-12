#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]

enum Stage {
    Planned,
    Running,
    Complete,
}

impl Stage {
    const ALL: [Self; 3] = [Self::Planned, Self::Running, Self::Complete];
}

fn main() {}
