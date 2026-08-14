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

#[derive(serde::Serialize)]
pub struct PublicRecord {
    value: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StableVariantsOnly {
    Updated { record_value: u64 },
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", rename_all_fields = "snake_case")]
pub enum StableVariantFields {
    Updated { record_value: u64 },
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum UntaggedPayload {
    Number { numeric_value: u64 },
    Empty,
}

#[derive(serde::Serialize)]
pub struct RawIdentifier {
    r#type: String,
}

mod private_api {
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum InternalEvent {
        Created,
        Deleted,
    }
}

fn main() {}
