#![allow(dead_code, unknown_lints)]

use serde::{Deserialize, Deserializer, Serializer};

#[derive(serde::Serialize, serde::Deserialize)]
struct Record {
    #[serde(
        serialize_with = "timestamp_seconds",
        deserialize_with = "timestamp_milliseconds"
    )]
    created_at: u64,
}

fn timestamp_seconds<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(*value)
}

fn timestamp_milliseconds<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer)
}

fn main() {}
