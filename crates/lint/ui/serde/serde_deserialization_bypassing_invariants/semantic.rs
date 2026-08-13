#![allow(dead_code, unknown_lints)]

#[derive(Debug)]
struct OutOfRange;

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

fn main() {}
