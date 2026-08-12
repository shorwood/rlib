#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(dead_code, misordered_module_declarations)]
#![register_tool(rlib_lint)]

#[derive(Clone, Copy)]
pub enum Stage {
    Planned,
    Running,
    Complete,
}

pub const STAGES: &[Stage] = &[Stage::Planned, Stage::Running, Stage::Complete];

impl Stage {
    const ALL: [Self; 3] = [Self::Planned, Self::Running, Self::Complete];
}

const WRONG_ORDER: [Stage; 3] = [Stage::Running, Stage::Planned, Stage::Complete];

fn main() {}
