#![allow(dead_code, unknown_lints)]

#[derive(serde::Serialize, serde::Deserialize)]
struct Details {
    id: u64,
    label: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Envelope {
    id: u64,
    #[serde(flatten)]
    details: Details,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SafeEnvelope {
    envelope_id: u64,
    #[serde(flatten)]
    details: Details,
}

fn main() {}
