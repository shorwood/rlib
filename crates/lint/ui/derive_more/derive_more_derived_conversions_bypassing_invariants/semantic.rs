#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[derive(derive_more::From)]
pub struct EmailAddress(String);

impl EmailAddress {
    fn parse(value: String) -> Result<Self, &'static str> {
        value
            .contains('@')
            .then_some(Self(value))
            .ok_or("missing at sign")
    }
}

#[derive(derive_more::From)]
struct Plain(String);

#[derive(derive_more::From)]
struct Open(pub String);

impl Open {
    fn parse(value: String) -> Result<Self, &'static str> {
        Ok(Self(value))
    }
}

#[derive(derive_more::From)]
pub struct CrateRestricted(pub(crate) String);

impl CrateRestricted {
    fn parse(value: String) -> Result<Self, &'static str> {
        if value.is_empty() {
            Err("empty value")
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(derive_more::TryFrom)]
#[try_from(repr)]
#[repr(u8)]
enum ReprConversion {
    Unit,
    Value(String),
}

impl ReprConversion {
    fn parse(value: String) -> Result<Self, &'static str> {
        if value.is_empty() {
            Err("empty value")
        } else {
            Ok(Self::Value(value))
        }
    }
}

fn main() {}
