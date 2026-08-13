#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[derive(derive_more::Display, derive_more::FromStr)]
#[display("port:{_0}")]
struct Port(u16);

#[derive(derive_more::Display, derive_more::FromStr)]
struct Transparent(u16);

#[derive(derive_more::Display, derive_more::FromStr)]
#[display("key:{_0}")]
struct StringKey(String);

fn main() {}
