#![allow(
    dead_code,
    misordered_inherent_impl_items,
    misordered_module_declarations,
    misordered_type_declarations,
    non_adjacent_struct_impls
)]
#![warn(missing_section_dividers)]

struct Missing;
impl Missing {}

// -----------------------------------------------------------------------------
// Covered: Covered model and behavior
// -----------------------------------------------------------------------------

struct Covered;
impl Covered {}

mod nested {
    struct NestedMissing;
}

macro_rules! generated_type {
    () => {
        struct Generated;
    };
}

generated_type!();

fn main() {}
