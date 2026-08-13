#![allow(dead_code, unknown_lints)]

#[derive(serde::Serialize, serde::Deserialize)]
struct Account {
    #[serde(rename(serialize = "user_id", deserialize = "id"))]
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MigratingAccount {
    /// Writes the new schema name while accepting the legacy input name.
    #[serde(rename(serialize = "user_id", deserialize = "id"))]
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StableAccount {
    #[serde(rename = "user_id")]
    id: u64,
}

fn main() {}
