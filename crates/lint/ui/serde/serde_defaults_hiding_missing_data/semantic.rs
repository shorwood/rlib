#![allow(dead_code, unknown_lints)]

#[derive(serde::Deserialize)]
struct Limits {
    #[serde(default)]
    maximum: u32,
    #[serde(default)]
    label: Option<String>,
    #[serde(default = "default_maximum")]
    preferred_maximum: u32,
    /// Zero is the documented legacy representation for an absent minimum.
    #[serde(default)]
    minimum: u32,
}

fn default_maximum() -> u32 {
    100
}

fn main() {}
