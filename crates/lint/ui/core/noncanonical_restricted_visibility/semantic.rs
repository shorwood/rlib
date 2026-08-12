#![warn(noncanonical_restricted_visibility)]
#![allow(dead_code)]

pub(self) struct SelfVisible;

mod parent {
    mod child {
        pub(in crate::parent) struct PathRestricted {
            pub(in crate::parent) value: usize,
        }
    }

    pub(in super) const PARENT_VALUE: usize = 1;
    pub(in crate) static CRATE_VALUE: usize = 2;
}

mod canonical {
    pub(super) struct Canonical {
        pub(super) parent: usize,
        pub(crate) crate_wide: usize,
        public: usize,
    }

    impl Canonical {
        pub(in crate) fn noncanonical_method(&self) {}

        pub(crate) fn canonical_method(&self) {}
    }
}

pub(in self) fn in_self() {}

fn main() {}
