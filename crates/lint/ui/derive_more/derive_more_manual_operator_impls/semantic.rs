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

struct Normalized(i64);

impl Sub for Normalized {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self((self.0 - rhs.0).max(0))
    }
}

fn main() {}
