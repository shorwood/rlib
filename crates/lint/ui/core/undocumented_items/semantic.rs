#![warn(rlib::undocumented_items)]
#![allow(dead_code, rlib::misordered_inherent_impl_items, rlib::misordered_module_declarations)]

use documented_module::Documented;

struct MissingStruct {
    missing_field: usize,
}

enum MissingEnum {
    MissingVariant { missing_variant_field: usize },
}

trait MissingTrait {
    fn missing_trait_method(&self);
}

fn missing_function() {}

#[doc(hidden)]
struct HiddenMetadataIsNotDocumentation;

#[doc = "An attribute-provided documentation contract."]
struct AttributeDocumented;

/// Type whose inherent item is checked independently.
struct DocumentedType;

impl DocumentedType {
    fn missing_method(&self) {}
}

impl MissingTrait for DocumentedType {
    fn missing_trait_method(&self) {}
}

#[allow(rlib::undocumented_items)]
struct ExplicitlyAllowed;

mod documented_module {
    /// A declaration proving that modules and imports need no barrel documentation.
    pub struct Documented;
}

/// Exercises the documented import without requiring module documentation.
fn main() {
    let _ = Documented;
}

macro_rules! local_undocumented_item {
    () => {
        struct LocalMacroItem;
    };
}

local_undocumented_item!();
