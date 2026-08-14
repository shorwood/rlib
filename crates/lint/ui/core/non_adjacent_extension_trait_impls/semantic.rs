// aux-build: foreign_types.rs
// edition:2024

#![warn(non_adjacent_extension_trait_impls)]
#![allow(dead_code, non_adjacent_struct_impls, unused_variables)]

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

trait BlanketExt {
    fn inspect(&self);
}

impl<T> BlanketExt for [T] {
    fn inspect(&self) {}
}

struct Local<T>(T);

trait LocalTrait {
    fn inspect(&self);
}

struct LocalInterruption;

// A generic local nominal target does not turn an ordinary trait into an extension trait.
impl<T> LocalTrait for Local<T> {
    fn inspect(&self) {}
}

trait InterruptedExt {
    fn inspect(&self);
}

struct Interruption;

impl InterruptedExt for InterruptedTarget {
    fn inspect(&self) {}
}

trait InterruptedBlanketExt {
    fn inspect(&self);
}

struct BlanketInterruption;

impl<T> InterruptedBlanketExt for &mut T {
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
