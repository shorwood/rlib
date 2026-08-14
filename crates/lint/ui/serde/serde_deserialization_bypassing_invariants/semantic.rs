#![allow(dead_code, unknown_lints)]

#[derive(Debug)]
struct OutOfRange;

impl std::fmt::Display for OutOfRange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("out of range")
    }
}

#[derive(serde::Deserialize)]
struct Percentage(u8);

impl Percentage {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err(OutOfRange)
        }
    }
}

#[derive(serde::Deserialize)]
struct InputPercentage(u8);

#[derive(serde::Deserialize)]
#[serde(try_from = "u8")]
struct ValidatedPercentage(u8);

impl TryFrom<u8> for ValidatedPercentage {
    type Error = OutOfRange;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl ValidatedPercentage {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err(OutOfRange)
        }
    }
}

#[derive(serde::Deserialize)]
struct PublicData(pub u8);

impl PublicData {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        Ok(Self(value))
    }
}

fn main() {}
