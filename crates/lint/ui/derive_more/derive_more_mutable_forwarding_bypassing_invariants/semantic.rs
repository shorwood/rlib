#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[derive(derive_more::Deref, derive_more::DerefMut)]
struct SortedValues(Vec<u32>);

impl SortedValues {
    fn checked(values: Vec<u32>) -> Result<Self, &'static str> {
        values
            .windows(2)
            .all(|pair| pair[0] <= pair[1])
            .then_some(Self(values))
            .ok_or("values are not sorted")
    }
}

#[derive(derive_more::AsMut)]
struct Bounded(Vec<u32>);

impl Bounded {
    fn checked(values: Vec<u32>) -> Result<Self, &'static str> {
        (values.len() <= 4)
            .then_some(Self(values))
            .ok_or("too many values")
    }
}

#[derive(derive_more::Deref, derive_more::DerefMut)]
struct Plain(Vec<u32>);

fn main() {}
