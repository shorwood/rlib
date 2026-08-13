#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::num::ParseIntError;
use std::str::FromStr;

struct Port(u16);

impl FromStr for Port {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

struct Trimmed(u16);

impl FromStr for Trimmed {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.trim().parse().map(Self)
    }
}

struct Validated(u16);

impl FromStr for Validated {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse::<u16>().map(|value| Self(value.max(1)))
    }
}

fn main() {}
