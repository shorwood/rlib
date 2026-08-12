#![warn(undocumented_items)]
#![allow(dead_code, misordered_inherent_impl_items, misordered_module_declarations)]

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

/// Type whose inherent item is checked independently.
struct DocumentedType;

impl DocumentedType {
    fn missing_method(&self) {}
}

impl MissingTrait for DocumentedType {
    fn missing_trait_method(&self) {}
}

#[allow(undocumented_items)]
struct ExplicitlyAllowed;

mod documented_module {
    /// A declaration proving that modules and imports need no barrel documentation.
    pub struct Documented;
}

/// Exercises the documented import without requiring module documentation.
fn main() {
    let _ = Documented;
}
