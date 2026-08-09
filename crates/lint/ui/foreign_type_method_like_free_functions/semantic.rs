// aux-build: foreign_types.rs
// edition:2024

#![warn(foreign_type_method_like_free_functions)]
#![allow(dead_code, misordered_module_declarations, unused_variables)]

extern crate foreign_types;

use foreign_types::{Ambient, Document, ExternalItem, ExternalType, Registry, external_helper};

type ItemAlias = ExternalItem;

// `Ambient` recurs beside distinct subjects and must not become the proposed owner.
pub fn inspect_item(ambient: &Ambient, item: &ExternalItem) {}
pub(crate) fn inspect_type(ambient: &Ambient, ty: &ExternalType) {}

// Both foreign types remain plausible, so the lint must report without guessing.
pub fn register_document(registry: &Registry, document: &Document) {}

// A single foreign nominal parameter is an unambiguous extension subject.
pub fn inspect(item: &ExternalItem) {}
pub fn inspect_alias(item: &ItemAlias) {}

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
