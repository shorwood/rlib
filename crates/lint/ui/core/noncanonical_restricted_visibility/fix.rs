// run-rustfix
#![warn(noncanonical_restricted_visibility)]
#![allow(dead_code)]

pub(self) struct PrivateValue;

mod parent {
    pub(in super) const PARENT_VALUE: usize = 1;

    pub(in crate) static CRATE_VALUE: usize = 2;
}

fn main() {}
