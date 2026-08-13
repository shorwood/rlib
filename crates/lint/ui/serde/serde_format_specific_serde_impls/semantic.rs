#![allow(dead_code, unknown_lints)]

use serde::{Serialize, Serializer};

struct Identifier(u64);

impl Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.0.to_string())
        } else {
            serializer.serialize_u64(self.0)
        }
    }
}

struct StableIdentifier(u64);

impl Serialize for StableIdentifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

fn main() {}
