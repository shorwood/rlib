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

fn main() {}
