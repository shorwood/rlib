#![allow(dead_code, rlib::misordered_module_declarations, unknown_lints)]

use std::hash::{Hash, Hasher};

#[derive(Hash, derive_more::PartialEq)]
struct Key {
    id: u64,
    #[partial_eq(skip)]
    revision: u64,
}

#[derive(Hash, derive_more::PartialEq)]
struct Consistent {
    id: u64,
    revision: u64,
}

#[derive(derive_more::PartialEq)]
struct CustomHash {
    id: u64,
    #[partial_eq(skip)]
    cached: u64,
}

impl Hash for CustomHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[derive(Hash, Eq, Ord, PartialOrd, derive_more::PartialEq)]
struct OrderedKey {
    id: u64,
    #[partial_eq(skip)]
    revision: u64,
}

#[derive(Hash, derive_more::PartialEq)]
enum EventKey {
    Unit,
    Revision {
        id: u64,
        #[partial_eq(skip)]
        revision: u64,
    },
}

fn main() {}
