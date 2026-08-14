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

struct Empty;

impl Empty {
    pub const fn new() -> Self {
        Self
    }
}

struct GenericPoint<T> {
    x: T,
    y: T,
}

impl<T> GenericPoint<T> {
    pub const fn new(x: T, y: T) -> GenericPoint<T> {
        GenericPoint { x, y }
    }
}

struct Reordered {
    x: i32,
    y: i32,
}

impl Reordered {
    // False-positive boundary: derive_more orders arguments by field declaration order.
    pub const fn new(y: i32, x: i32) -> Self {
        Self { x, y }
    }
}

struct Coercing<'a> {
    value: &'a [u8],
}

impl<'a> Coercing<'a> {
    // False-positive boundary: the derive accepts `&[u8]`, not this coercible array-ref API.
    pub const fn new(value: &'a [u8; 4]) -> Self {
        Self { value }
    }
}

trait Marker {}

struct Opaque;

impl Marker for Opaque {}

impl Opaque {
    // False-positive boundary: the derive returns `Opaque`, not this opaque public contract.
    pub const fn new() -> impl Marker {
        Self
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
