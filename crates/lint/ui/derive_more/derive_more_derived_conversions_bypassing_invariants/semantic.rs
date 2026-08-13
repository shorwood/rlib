#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[derive(derive_more::From)]
struct EmailAddress(String);

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

fn main() {}
