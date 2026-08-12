#![allow(dead_code, unknown_lints)]

use serde::Serialize;

#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(derive(Serialize))]
enum Event {
    Created(String),
    Deleted(u64),
}

fn main() {}
