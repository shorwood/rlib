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

#[derive(serde::Serialize, serde::Deserialize)]
struct TupleRecord(
    #[serde(
        serialize_with = "timestamp_seconds",
        deserialize_with = "timestamp_milliseconds"
    )]
    u64,
);

#[derive(serde::Serialize, serde::Deserialize)]
enum Event {
    Created {
        #[serde(
            serialize_with = "timestamp_seconds",
            deserialize_with = "timestamp_milliseconds"
        )]
        at: u64,
    },
}

mod timestamp_seconds_adapter {
    use super::*;

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(*value)
    }
}

mod timestamp_milliseconds_adapter {
    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        u64::deserialize(deserializer)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ModuleAdapters {
    #[serde(
        serialize_with = "timestamp_seconds_adapter::serialize",
        deserialize_with = "timestamp_milliseconds_adapter::deserialize"
    )]
    at: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SkippedAdapter {
    #[serde(
        serialize_with = "timestamp_seconds",
        deserialize_with = "timestamp_milliseconds",
        skip_deserializing
    )]
    at: u64,
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
