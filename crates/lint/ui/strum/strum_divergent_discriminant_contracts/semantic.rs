#![allow(dead_code, unknown_lints)]

use serde::{Deserialize, Serialize};

#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(derive(Serialize))]
enum Event {
    Created(String),
    Deleted(u64),
}

#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(derive(Deserialize))]
enum InputEvent {
    Created(String),
    Deleted(u64),
}

#[derive(strum::EnumDiscriminants)]
enum ManualEvent {
    Created(String),
    Deleted(u64),
}

impl Serialize for ManualEventDiscriminants {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_unit_variant("ManualEventDiscriminants", 0, "Created")
    }
}

#[derive(strum::EnumDiscriminants)]
enum InternalEvent {
    Created(String),
    Deleted(u64),
}

fn main() {}
