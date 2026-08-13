#![allow(dead_code, unknown_lints)]

#[derive(serde::Serialize, serde::Deserialize)]
pub enum ApiEvent {
    UserCreated,
    UserDeleted,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StableEvent {
    UserCreated,
    UserDeleted,
}

fn main() {}
