// aux-build: foreign_types.rs
// edition:2024

#![warn(foreign_type_method_like_free_functions)]
#![allow(
    dead_code,
    method_like_free_functions,
    misordered_module_declarations,
    unused_variables
)]

extern crate foreign_types;

use foreign_types::{Ambient, Document, ExternalItem, ExternalType, Registry, external_helper};

type ItemAlias = ExternalItem;

pub struct LocalSubject;

// `Ambient` recurs beside distinct subjects and must not become the proposed owner.
pub fn inspect_item(ambient: &Ambient, item: &ExternalItem) {}
pub(crate) fn inspect_type(ambient: &Ambient, ty: &ExternalType) {}

// Both foreign types remain plausible, so the lint must report without guessing.
pub fn register_document(registry: &Registry, document: &Document) {}

// False-negative boundary: direct, aliased, and repeated parameters resolve to one foreign owner.
pub fn inspect(item: &ExternalItem) {}
pub fn inspect_alias(item: &ItemAlias) {}
pub fn inspect_repeated(item: &ExternalItem, duplicate: &ExternalItem) {}

// False-positive boundaries: local and primitive parameters do not imply a foreign owner.
pub fn inspect_local(item: &LocalSubject) {}
pub fn inspect_count(count: usize) {}

macro_rules! local_helper {
    () => {
        pub fn generated_locally(item: &ExternalType) {}
    };
}

local_helper!();
external_helper!();

#[unsafe(no_mangle)]
pub extern "Rust" fn dylint_entry(registry: &Registry, document: &Document) {}

mod private_api {
    use super::{ExternalItem, ExternalType};

    fn private_helper(item: &ExternalItem) {}
    pub(self) fn self_visible_helper(ty: &ExternalType) {}
}

mod restricted_api {
    use super::{Document, ExternalType};

    pub(super) fn super_visible(ty: &ExternalType) {}
    pub(in crate) fn crate_visible(document: &Document) {}
}

fn main() {}

// False-positive boundaries: local inputs and local success returns provide a stronger owner.
pub fn combine_local(local: &LocalSubject, external: &ExternalItem) {}
pub fn load_local(external: &ExternalItem) -> Result<LocalSubject, ()> {
    Ok(LocalSubject)
}
