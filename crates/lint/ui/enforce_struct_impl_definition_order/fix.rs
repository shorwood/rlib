// run-rustfix
// rustfix-only-machine-applicable
// edition:2024

#![feature(register_tool)]
#![allow(dead_code, enforce_inherent_impl_item_order, enforce_module_declaration_order, enforce_type_dependency_order)]
#![register_tool(rlib_lint)]

trait Behavior {}

struct Movable;
fn movable_separator() {}
impl Movable {
    fn value(&self) -> u8 {
        1
    }
}

struct Multiple;
fn multiple_separator() {}
impl Multiple {}
impl Behavior for Multiple {}

struct Split;
impl Split {}
fn split_separator() {}
impl Behavior for Split {}

fn main() {}
