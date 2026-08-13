#![allow(dead_code, unknown_lints)]

struct Source {
    id: u64,
    label: String,
}

#[derive(serde::Serialize)]
#[serde(remote = "Source")]
struct SourceDef {
    id: u64,
}

struct StableSource {
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "StableSource")]
struct StableSourceDef {
    id: u64,
}

fn main() {}
