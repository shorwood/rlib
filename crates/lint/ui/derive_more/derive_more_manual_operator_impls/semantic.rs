#![allow(dead_code, unknown_lints)]

use std::ops::{Add, AddAssign, Neg, Sub};

struct Offset(i64);

impl Add for Offset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

struct Delta(i64);

impl Neg for Delta {
    type Output = Self;

    fn neg(self) -> Self {
        Self(-self.0)
    }
}

struct Counter(i64);

impl AddAssign for Counter {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

struct Generic<T>(T);

impl<T: Add<Output = T>> Add for Generic<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

struct Named {
    value: i64,
}

impl Neg for Named {
    type Output = Self;

    fn neg(self) -> Self {
        Self { value: -self.value }
    }
}

struct Other(i64);
struct Mixed(i64);

impl Add<Other> for Mixed {
    type Output = Self;

    // False-positive boundary: a plain derive does not reproduce this heterogeneous RHS.
    fn add(self, rhs: Other) -> Self {
        Self(self.0 + rhs.0)
    }
}

struct Documented(i64);

impl Neg for Documented {
    type Output = Self;

    /// This operator is an authored compatibility contract.
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

struct Normalized(i64);

impl Sub for Normalized {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self((self.0 - rhs.0).max(0))
    }
}

fn main() {}
