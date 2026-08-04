// aux-build: external_macro.rs
// edition:2024

#![feature(register_tool)]
#![allow(dead_code, enforce_inherent_impl_item_order, enforce_module_declaration_order, enforce_struct_impl_definition_order, enforce_type_dependency_order)]
#![register_tool(rlib_lint)]

extern crate external_macro;

use external_macro::external_impl;

#[path = "support/implementations.inc"]
mod implementations;
#[path = "support/owner.inc"]
mod owner;

trait Behavior {}
trait ReferenceBehavior {}

// Different modules are allowed when both declarations are in this physical file.
mod same_file_owner {
    pub(super) struct Local;
}

mod same_file_implementation {
    impl super::same_file_owner::Local {}
}

// Reference self-types and non-struct self-types are outside this rule.
impl ReferenceBehavior for &owner::Referenced {}
impl Behavior for owner::Choice {}

// External macro output is not editable and remains ignored.
external_impl!(owner::ExternalMacroTarget);

fn main() {}
