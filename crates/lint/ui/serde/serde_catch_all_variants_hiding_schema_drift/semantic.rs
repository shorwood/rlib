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

mod private_schema {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum InternalPermission {
        Read,
        #[serde(other)]
        Unknown,
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputOnlyPermission {
    Read,
    #[serde(other)]
    Unknown,
}

fn main() {}
