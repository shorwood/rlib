#![allow(dead_code, rlib::misordered_module_declarations, unknown_lints)]

#[derive(derive_more::Constructor)]
pub struct Percentage(u8);

impl Percentage {
    fn checked(value: u8) -> Result<Self, &'static str> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err("percentage exceeds 100")
        }
    }
}

#[derive(derive_more::Constructor)]
struct Plain(u8);

#[derive(derive_more::Constructor)]
struct Open(pub u8);

impl Open {
    fn checked(value: u8) -> Result<Self, &'static str> {
        Ok(Self(value))
    }
}

#[derive(derive_more::Constructor)]
struct InfallibleResult(u8);

impl InfallibleResult {
    fn checked(value: u8) -> Result<Self, &'static str> {
        Ok(Self(value))
    }
}

fn main() {}
