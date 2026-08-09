// aux-build: foreign_types.rs
// edition:2024

#![warn(non_adjacent_extension_trait_impls)]
#![allow(dead_code, unused_variables)]

extern crate foreign_types;

use foreign_types::{AcceptedTarget, AdditionalTarget, InterruptedTarget, RemoteTarget};

trait AcceptedExt {
    fn inspect(&self);
}

// Comments and attributes do not interrupt the declaration group.
#[doc = "Accepted extension implementation."]
impl AcceptedExt for AcceptedTarget {
    fn inspect(&self) {}
}

impl AcceptedExt for AdditionalTarget {
    fn inspect(&self) {}
}

trait InterruptedExt {
    fn inspect(&self);
}

struct Interruption;

impl InterruptedExt for InterruptedTarget {
    fn inspect(&self) {}
}

trait RemoteExt {
    fn inspect(&self);
}

mod remote {
    use super::{RemoteExt, RemoteTarget};

    impl RemoteExt for RemoteTarget {
        fn inspect(&self) {}
    }
}

fn main() {}
