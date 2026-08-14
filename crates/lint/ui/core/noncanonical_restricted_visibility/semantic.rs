#![warn(noncanonical_restricted_visibility)]
#![allow(dead_code)]

pub(self) struct SelfVisible;
pub(in crate) type CrateAlias = usize;

pub(in crate) mod restricted_module {}

struct TupleField(pub(in crate) usize);

union RestrictedUnion {
    pub(in crate) value: usize,
}

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
        pub(in crate) const NONCANONICAL_CONSTANT: usize = 1;

        pub(in crate) fn noncanonical_method(&self) {}

        pub(crate) fn canonical_method(&self) {}
    }
}

pub(in self) fn in_self() {}

macro_rules! generated_visibility {
    () => {
        pub(in crate) struct GeneratedVisibility;
    };
}

generated_visibility!();

fn main() {}
