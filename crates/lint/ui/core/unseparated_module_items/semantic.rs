#![allow(dead_code)]
#![warn(rlib::unseparated_module_items)]

struct DenseType;
// This comment occupies the declaration boundary, so no automatic edit is safe.
impl DenseType {}

struct AlreadySeparated;

impl AlreadySeparated {}

struct BeforeImport;
use std::fmt;
struct AfterImport;

struct BeforeMacro;
macro_rules! separator {
    () => {};
}
struct AfterMacro;

mod compact_first {}
mod compact_second {}

struct BeforeModule;
mod module_boundary {}
struct AfterModule;

macro_rules! generated_items {
    () => {
        struct Generated;
        impl Generated {}
    };
}

generated_items!();

fn main() {
    let _ = fmt::Error;
}
