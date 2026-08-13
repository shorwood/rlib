#![allow(dead_code, misordered_module_declarations, unknown_lints)]

struct Point {
    x: i32,
    y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

struct Coordinate(i32, i32);

impl Coordinate {
    pub const fn new(x: i32, y: i32) -> Self {
        Self(x, y)
    }
}

struct Checked(u32);

impl Checked {
    pub const fn new(value: u32) -> Self {
        Self(if value == 0 { 1 } else { value })
    }
}

struct Documented(u32);

impl Documented {
    /// Establishes the public construction contract.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

fn main() {}
