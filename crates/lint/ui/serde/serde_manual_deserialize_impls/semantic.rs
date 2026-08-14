#![allow(dead_code, unknown_lints)]

use serde::{Deserialize, Deserializer};

struct UserId(u64);

impl<'de> Deserialize<'de> for UserId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(u64::deserialize(deserializer)?))
    }
}

struct Bounded(u64);

impl<'de> Deserialize<'de> for Bounded {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Ok(Self(value.min(100)))
    }
}

struct Generic<T>(T);

impl<'de, T> Deserialize<'de> for Generic<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(T::deserialize(deserializer)?))
    }
}

struct NamedId {
    value: u64,
}

impl<'de> Deserialize<'de> for NamedId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self {
            value: u64::deserialize(deserializer)?,
        })
    }
}

struct QualifiedResult(u64);

impl<'de> Deserialize<'de> for QualifiedResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Result::Ok(Self(u64::deserialize(deserializer)?))
    }
}

struct DocumentedImpl(u64);

/// This implementation decodes an identifier.
impl<'de> Deserialize<'de> for DocumentedImpl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(u64::deserialize(deserializer)?))
    }
}

fn main() {}
