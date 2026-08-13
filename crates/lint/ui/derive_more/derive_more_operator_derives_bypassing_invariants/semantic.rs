#![allow(dead_code, unknown_lints)]

#[derive(Debug)]
struct OutOfRange;

#[derive(derive_more::Add, derive_more::Sum)]
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

#[derive(derive_more::Add)]
struct Offset(i64);

fn main() {}
