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

#[derive(serde::Serialize, serde::Deserialize)]
struct Leaf {
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Middle {
    #[serde(flatten)]
    leaf: Leaf,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RecursiveEnvelope {
    id: u64,
    #[serde(flatten)]
    middle: Middle,
}

#[derive(serde::Deserialize)]
struct AliasedDetails {
    #[serde(alias = "legacy_id")]
    id: u64,
}

#[derive(serde::Deserialize)]
struct AliasEnvelope {
    legacy_id: u64,
    #[serde(flatten)]
    details: AliasedDetails,
}

#[derive(serde::Serialize)]
struct LeftDetails {
    key: String,
}

#[derive(serde::Serialize)]
struct RightDetails {
    key: String,
}

#[derive(serde::Serialize)]
struct TwoFlattenedSchemas {
    #[serde(flatten)]
    left: LeftDetails,
    #[serde(flatten)]
    right: RightDetails,
}

fn main() {}
