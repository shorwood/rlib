#![allow(dead_code, misordered_module_declarations, unknown_lints)]

struct Record {
    id: u64,
    revision: u64,
    cached: String,
}

impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.revision == other.revision
    }
}

impl Eq for Record {}

struct GenericRecord<T> {
    value: T,
}

impl<T: PartialEq> PartialEq for GenericRecord<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

struct Empty;

impl PartialEq for Empty {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

struct DocumentedEquality(u32);

/// Equality here is an authored compatibility contract.
impl PartialEq for DocumentedEquality {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

struct DocumentedMethod(u32);

impl PartialEq for DocumentedMethod {
    /// Equality here is an authored compatibility contract.
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

struct Approximate(f64);

impl PartialEq for Approximate {
    fn eq(&self, other: &Self) -> bool {
        (self.0 - other.0).abs() < f64::EPSILON
    }
}

struct CustomNe(u32);

impl PartialEq for CustomNe {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn ne(&self, _other: &Self) -> bool {
        false
    }
}

fn main() {}
