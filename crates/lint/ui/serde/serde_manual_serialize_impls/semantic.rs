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

struct Generic<T>(T);

impl<T: Serialize> Serialize for Generic<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

struct NamedId {
    value: u64,
}

impl Serialize for NamedId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(serializer)
    }
}

struct QualifiedCall(u64);

impl Serialize for QualifiedCall {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        <u64 as Serialize>::serialize(&self.0, serializer)
    }
}

struct DocumentedImpl(u64);

/// This implementation serializes an identifier.
impl Serialize for DocumentedImpl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

fn main() {}
