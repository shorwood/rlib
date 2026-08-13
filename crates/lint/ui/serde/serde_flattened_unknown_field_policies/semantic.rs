#![allow(dead_code, unknown_lints)]

use std::collections::HashMap;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictRequest {
    id: u64,
    #[serde(flatten)]
    extensions: HashMap<String, String>,
}

#[derive(serde::Deserialize)]
struct ExtensibleRequest {
    id: u64,
    #[serde(flatten)]
    extensions: HashMap<String, String>,
}

fn main() {}
