#![allow(dead_code, rlib::misordered_module_declarations, unknown_lints)]

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

struct NamedSystemList {
    systems: Vec<u32>,
}

impl From<Vec<u32>> for NamedSystemList {
    fn from(systems: Vec<u32>) -> Self {
        Self { systems }
    }
}

impl From<NamedSystemList> for Vec<u32> {
    fn from(list: NamedSystemList) -> Self {
        list.systems
    }
}

struct Generic<T>(T);

impl<T> From<T> for Generic<T> {
    fn from(value: T) -> Self {
        Generic(value)
    }
}

struct Restricted<T>(T);

impl<T: Copy> From<T> for Restricted<T> {
    // False-positive boundary: deriving would add this conversion for every `T`.
    fn from(value: T) -> Self {
        Self(value)
    }
}

struct Documented(u32);

/// This conversion is the documented compatibility boundary.
impl From<u32> for Documented {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

struct MethodDocumented(u32);

impl From<u32> for MethodDocumented {
    /// This conversion has an authored contract.
    fn from(value: u32) -> Self {
        Self(value)
    }
}

struct Checked(u32);

impl From<u32> for Checked {
    fn from(value: u32) -> Self {
        Self(value.max(1))
    }
}

fn main() {}
