#![allow(dead_code, rlib::misordered_module_declarations, unknown_lints)]

#[derive(derive_more::Deref, derive_more::DerefMut)]
pub struct SortedValues(Vec<u32>);

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
pub struct Bounded(Vec<u32>);

impl Bounded {
    fn checked(values: Vec<u32>) -> Result<Self, &'static str> {
        (values.len() <= 4)
            .then_some(Self(values))
            .ok_or("too many values")
    }
}

#[derive(derive_more::Deref, derive_more::DerefMut)]
struct Plain(Vec<u32>);

#[derive(derive_more::AsMut)]
pub struct PublicStorage {
    #[as_mut]
    pub values: Vec<u32>,
    limit: usize,
}

impl PublicStorage {
    fn checked(values: Vec<u32>, limit: usize) -> Result<Self, &'static str> {
        if values.len() > limit {
            return Err("too many values");
        }
        Ok(Self { values, limit })
    }
}

fn main() {}
