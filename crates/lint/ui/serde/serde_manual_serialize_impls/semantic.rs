#![allow(dead_code, unknown_lints)]

use serde::{Serialize, Serializer};

struct UserId(u64);

impl Serialize for UserId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

struct Masked(u64);

impl Serialize for Masked {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        0_u64.serialize(serializer)
    }
}

fn main() {}
