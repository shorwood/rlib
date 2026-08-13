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
fn missing_parser() {}
fn missing_renderer() {}
fn missing_validator() {}
fn missing_loader() {}
fn missing_writer() {}

// -----------------------------------------------------------------------------
// Covered: Covered model and behavior
// -----------------------------------------------------------------------------

struct Covered;
impl Covered {}

mod nested {
    struct NestedMissing;
}

mod distinct_concepts {
    struct Reader;
    struct Writer;
}

mod cohesive_family {
    struct Request;
    struct RequestBuilder;
}

mod serde {
    mod contracts {
        struct SerdeAttributes;
        struct SerdeCase;
        struct SerdeContractCatalog;
    }
}

macro_rules! generated_type {
    () => {
        struct Generated;
    };
}

generated_type!();

fn main() {}
