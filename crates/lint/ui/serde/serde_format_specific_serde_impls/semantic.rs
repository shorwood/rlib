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

struct UnrelatedDocumentation(u64);

/// Identifier serialization.
impl Serialize for UnrelatedDocumentation {
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

struct MentionOnly(u64);

impl Serialize for MentionOnly {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // `serde_json::to_string` is intentionally only documentation here.
        let _ = "is_human_readable() serialize_str serialize_u64";
        serializer.serialize_u64(self.0)
    }
}

struct DocumentedDualShape(u64);

/// The human-readable and binary wire representations intentionally differ for schema compatibility.
impl Serialize for DocumentedDualShape {
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

fn main() {}
