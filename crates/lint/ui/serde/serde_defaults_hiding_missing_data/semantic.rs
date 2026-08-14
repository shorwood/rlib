#![allow(dead_code, unknown_lints)]

#[derive(serde::Deserialize)]
struct Limits {
    #[serde(default)]
    maximum: u32,
    #[serde(default)]
    label: std::option::Option<String>,
    #[serde(default = "default_maximum")]
    preferred_maximum: u32,
    /// Zero is the documented legacy representation for an absent minimum.
    #[serde(default)]
    minimum: u32,
}

#[derive(serde::Deserialize)]
struct UnrelatedDocumentation {
    /// The configured limit.
    #[serde(default)]
    maximum: u32,
}

#[derive(serde::Deserialize)]
struct TupleLimit(#[serde(default)] u32);

#[derive(serde::Deserialize)]
enum LimitEvent {
    Changed {
        #[serde(default)]
        maximum: u32,
    },
}

#[derive(serde::Deserialize)]
enum Option<T> {
    Some(T),
    None,
}

impl<T> Default for Option<T> {
    fn default() -> Self {
        Self::None
    }
}

#[derive(serde::Deserialize)]
struct CustomOption {
    #[serde(default)]
    value: Option<u32>,
}

#[derive(serde::Deserialize)]
struct SkippedInput {
    #[serde(default, skip_deserializing)]
    value: u32,
}

fn default_maximum() -> u32 {
    100
}

fn main() {}
