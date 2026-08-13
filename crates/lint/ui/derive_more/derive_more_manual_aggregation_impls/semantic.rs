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

fn main() {}
