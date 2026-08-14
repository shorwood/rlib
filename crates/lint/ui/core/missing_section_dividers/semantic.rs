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
fn missing_step_01() {}
fn missing_step_02() {}
fn missing_step_03() {}
fn missing_step_04() {}
fn missing_step_05() {}
fn missing_step_06() {}
fn missing_step_07() {}
fn missing_step_08() {}
fn missing_step_09() {}
fn missing_step_10() {}
fn missing_step_11() {}
fn missing_step_12() {}
fn missing_step_13() {}
fn missing_step_14() {}
fn missing_step_15() {}
fn missing_step_16() {}
fn missing_step_17() {}
fn missing_step_18() {}
fn missing_step_19() {}
fn missing_step_20() {}
fn missing_step_21() {}
fn missing_step_22() {}
fn missing_step_23() {}
fn missing_step_24() {}
fn missing_step_25() {}
fn missing_step_26() {}
fn missing_step_27() {}
fn missing_step_28() {}
fn missing_step_29() {}
fn missing_step_30() {}
fn missing_step_31() {}
fn missing_step_32() {}
fn missing_step_33() {}
fn missing_step_34() {}
fn missing_step_35() {}

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
