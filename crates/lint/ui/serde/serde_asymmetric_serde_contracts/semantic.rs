#![allow(dead_code, unknown_lints)]

#[derive(serde::Serialize, serde::Deserialize)]
struct Account {
    #[serde(rename(serialize = "user_id", deserialize = "id"))]
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MigratingAccount {
    /// Writes the new schema name while accepting the legacy input name.
    #[serde(rename(serialize = "user_id", deserialize = "id"))]
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StableAccount {
    #[serde(rename = "user_id")]
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UnrelatedDocumentation {
    /// The account identifier.
    #[serde(rename(serialize = "user_id", deserialize = "id"))]
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TupleAccount(#[serde(rename(serialize = "user_id", deserialize = "id"))] u64);

#[derive(serde::Serialize, serde::Deserialize)]
enum Event {
    Changed {
        #[serde(rename(serialize = "new_id", deserialize = "id"))]
        id: u64,
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
enum DirectionalVariant {
    #[serde(rename(serialize = "created", deserialize = "new"))]
    Created,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AliasBridge {
    #[serde(rename(serialize = "user_id", deserialize = "id"), alias = "user_id")]
    id: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SkippedDirection {
    #[serde(rename(serialize = "user_id", deserialize = "id"), skip_serializing)]
    id: u64,
}

fn main() {}
