#![allow(dead_code)]
#![warn(rlib::incoherent_type_family_names)]

macro_rules! generated_types {
    () => {
        struct GeneratedContext;
        struct GeneratedContextState;
    };
}

// -----------------------------------------------------------------------------
// GeneratedContext: Family naming fixture
// -----------------------------------------------------------------------------

generated_types!();

fn main() {}
