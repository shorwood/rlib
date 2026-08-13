#![allow(dead_code, unknown_lints)]

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Read,
    Write,
    #[serde(other)]
    Unknown,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrictPermission {
    Read,
    Write,
}

fn main() {}
