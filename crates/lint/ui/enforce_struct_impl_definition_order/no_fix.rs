// edition:2024

#![feature(register_tool)]
#![allow(dead_code, enforce_inherent_impl_item_order, enforce_module_declaration_order, enforce_struct_impl_colocation, enforce_type_dependency_order)]
#![register_tool(rlib_lint)]

// Attributes may change compilation or formatting and must travel deliberately.
struct Attributed;
fn attributed_separator() {}
#[doc = "This documentation belongs to the impl."]
impl Attributed {}

// The comment may describe the impl, so an automatic move must not orphan it.
struct Commented;
fn commented_separator() {}
// This behavior deliberately has an explanation attached.
impl Commented {}

// Moving across a macro definition can change which macro a method resolves.
struct MacroCrossing;
macro_rules! value {
    () => {
        1
    };
}
fn macro_separator() {}
impl MacroCrossing {
    fn value(&self) -> u8 {
        value!()
    }
}

// Included source is not owned by a one-file editor suggestion.
struct Included;
fn included_separator() {}
include!("included_impl.inc");

fn main() {}
