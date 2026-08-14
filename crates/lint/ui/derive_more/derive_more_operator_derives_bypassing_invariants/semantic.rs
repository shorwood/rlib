#![allow(dead_code, unknown_lints)]

#[derive(Debug)]
struct OutOfRange;

#[derive(derive_more::Add, derive_more::Sum)]
pub struct Percentage(u8);

impl Percentage {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err(OutOfRange)
        }
    }

    fn try_new(value: u8) -> Result<Self, OutOfRange> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err(OutOfRange)
        }
    }
}

#[derive(derive_more::SubAssign, derive_more::Neg, derive_more::Mul)]
pub struct Signed(i64);

impl Signed {
    fn new(value: i64) -> Result<Self, OutOfRange> {
        if value == i64::MIN {
            Err(OutOfRange)
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(derive_more::Add)]
pub struct PublicStorage(pub u8);

impl PublicStorage {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        if value == 0 {
            Err(OutOfRange)
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(derive_more::Add)]
pub struct OkOnly(u8);

impl OkOnly {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        Ok(Self(value))
    }
}

#[derive(derive_more::Add)]
struct PrivateScope(u8);

impl PrivateScope {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        if value == 0 {
            Err(OutOfRange)
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(derive_more::Add)]
struct Offset(i64);

fn main() {}
