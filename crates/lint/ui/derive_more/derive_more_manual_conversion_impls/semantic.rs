#![allow(dead_code, misordered_module_declarations, unknown_lints)]

struct SystemList(Vec<u32>);

impl From<Vec<u32>> for SystemList {
    fn from(systems: Vec<u32>) -> Self {
        Self(systems)
    }
}

impl From<SystemList> for Vec<u32> {
    fn from(systems: SystemList) -> Self {
        systems.0
    }
}

struct Checked(u32);

impl From<u32> for Checked {
    fn from(value: u32) -> Self {
        Self(value.max(1))
    }
}

fn main() {}
