
#![feature(register_tool)]
#![allow(dead_code, unused_imports, rlib::misordered_inherent_impl_items, rlib::misordered_module_declarations, rlib::misordered_type_declarations)]
#![register_tool(rlib_lint)]

// These path attributes deliberately load physical barrel files. The rule is about the source
// filename, so an inline module in this test driver would not exercise the policy.
#[path = "auxiliary/bad/mod.rs"]
mod bad;
#[path = "auxiliary/directory_bad/mod.rs"]
mod directory_bad;
#[path = "auxiliary/documented_leaf/mod.rs"]
mod documented_leaf;
#[path = "auxiliary/empty/mod.rs"]
mod empty;
#[path = "auxiliary/good/mod.rs"]
mod good;
#[path = "auxiliary/lib_bad/lib.rs"]
mod lib_bad;
#[path = "auxiliary/lib_good/lib.rs"]
mod lib_good;

// Ordinary Rust files may contain all of the constructs forbidden in barrel files.
use std::fmt;

struct OrdinaryFile;

impl OrdinaryFile {
    fn work() {}
}

macro_rules! ordinary_macro {
    () => {};
}

ordinary_macro!();

fn main() {}
