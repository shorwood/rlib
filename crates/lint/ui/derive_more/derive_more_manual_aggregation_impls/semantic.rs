#![allow(dead_code, unknown_lints)]

use std::iter::{Product, Sum};

struct Total(u64);

impl Sum for Total {
    fn sum<I: Iterator<Item = Self>>(values: I) -> Self {
        Self(values.map(|value| value.0).sum())
    }
}

struct Scale(u64);

impl Product for Scale {
    fn product<I: Iterator<Item = Self>>(values: I) -> Self {
        Self(values.map(|value| value.0).product())
    }
}

struct CheckedTotal(u64);

impl Sum for CheckedTotal {
    fn sum<I: Iterator<Item = Self>>(values: I) -> Self {
        Self(values.map(|value| value.0).fold(0, u64::saturating_add))
    }
}

struct NamedTotal {
    value: u64,
}

impl Sum for NamedTotal {
    fn sum<I: Iterator<Item = Self>>(values: I) -> Self {
        Self {
            value: values.map(|value| value.value).sum(),
        }
    }
}

struct GenericTotal<T>(T);

impl<T: Sum<T>> Sum for GenericTotal<T> {
    fn sum<I: Iterator<Item = Self>>(values: I) -> Self {
        Self(values.map(|value| value.0).sum())
    }
}

struct DocumentedTotal(u64);

/// The implementation intentionally uses the ordinary additive identity.
impl Sum for DocumentedTotal {
    fn sum<I: Iterator<Item = Self>>(values: I) -> Self {
        DocumentedTotal(values.map(|value| value.0).sum())
    }
}

struct BorrowedTotal(u64);

impl<'value> Sum<&'value BorrowedTotal> for BorrowedTotal {
    fn sum<I: Iterator<Item = &'value BorrowedTotal>>(values: I) -> Self {
        Self(values.map(|value| value.0).sum())
    }
}

fn main() {}
