#![allow(dead_code, unknown_lints)]

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Value {
    Detailed {
        id: u64,
        #[serde(default)]
        name: String,
    },
    Minimal {
        id: u64,
    },
}

#[derive(serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum DisjointValue {
    Text { text: String },
    Number { number: u64 },
}

fn main() {}
