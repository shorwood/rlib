#![allow(dead_code, unknown_lints)]

#[derive(serde::Serialize)]
enum FactoredNames {
    #[serde(rename = "userCreated")]
    UserCreated,
    #[serde(rename = "userDeleted")]
    UserDeleted,
    #[serde(rename = "legacy")]
    LegacyMode,
}

#[derive(serde::Serialize, serde::Deserialize)]
enum FactoredBidirectionalNames {
    #[serde(rename = "userCreated")]
    UserCreated,
    #[serde(rename = "userDeleted")]
    UserDeleted,
    #[serde(rename = "legacy")]
    LegacyMode,
}

#[derive(serde::Serialize)]
enum FactoredFields {
    Current {
        #[serde(rename = "firstValue")]
        first_value: u8,
        #[serde(rename = "secondValue")]
        second_value: u8,
    },
    Legacy {
        #[serde(rename = "legacy")]
        legacy_mode: u8,
    },
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
enum RedundantOverride {
    #[serde(rename = "userCreated")]
    UserCreated,
    UserDeleted,
}

#[derive(serde::Deserialize)]
struct RedundantAlias {
    #[serde(alias = "value")]
    value: u64,
}

#[derive(serde::Serialize)]
struct PrivateIdentity {
    #[serde(rename = "value")]
    value: u64,
}

#[derive(serde::Serialize)]
pub struct PublicIdentity {
    #[serde(rename = "value")]
    value: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DirectionalShorthand {
    #[serde(rename(serialize = "wireValue", deserialize = "wireValue"))]
    value: u64,
}

#[derive(serde::Serialize)]
struct IdenticalInactivePrivateDirection {
    #[serde(rename(serialize = "wireValue", deserialize = "wireValue"))]
    value: u64,
}

#[derive(serde::Serialize)]
struct InactivePrivateDirection {
    #[serde(rename(serialize = "wireValue", deserialize = "legacyValue"))]
    value: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum FullyInheritedDirectionalOverride {
    #[serde(rename(serialize = "userCreated", deserialize = "userCreated"))]
    UserCreated,
    UserDeleted,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SkippedDirectionalBranch {
    #[serde(
        skip_deserializing,
        rename(serialize = "wireValue", deserialize = "wireValue")
    )]
    value: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct OnlySkippedDirectionalBranch {
    #[serde(skip_deserializing, rename(deserialize = "legacyValue"))]
    value: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FullySkippedMemberRename {
    #[serde(skip, rename = "wireValue")]
    value: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkippedInheritedDirectionalBranches {
    #[serde(
        skip_deserializing,
        rename(serialize = "userCreated", deserialize = "legacyValue")
    )]
    user_created: u64,
}

#[derive(serde::Deserialize)]
struct OnlySkippedDerivedDirection {
    #[serde(
        skip_deserializing,
        rename(serialize = "wireValue", deserialize = "legacyValue")
    )]
    value: u64,
}

#[derive(serde::Serialize)]
pub struct PreservedPublicDirection {
    #[serde(rename(serialize = "wireValue", deserialize = "legacyValue"))]
    value: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
enum DirectionallyMeaningfulShorthand {
    #[serde(rename = "userCreated")]
    UserCreated,
    UserDeleted,
}

fn main() {}
