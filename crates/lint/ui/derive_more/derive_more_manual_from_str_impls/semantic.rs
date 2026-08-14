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

struct NamedPort {
    value: u16,
}

impl FromStr for NamedPort {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(|value| Self { value })
    }
}

struct Generic<T>(T);

impl<T: FromStr> FromStr for Generic<T> {
    type Err = T::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        T::from_str(value).map(Self)
    }
}

struct Documented(u16);

/// This parser is an authored compatibility contract.
impl FromStr for Documented {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

fn from_str(value: &str) -> Result<u16, ParseIntError> {
    value.trim().parse()
}

struct CustomFunction(u16);

impl FromStr for CustomFunction {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // False-positive boundary: terminal spelling does not make this the standard trait call.
        from_str(value).map(Self)
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
