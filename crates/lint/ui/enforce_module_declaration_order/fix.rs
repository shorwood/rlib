// run-rustfix
// rustfix-only-machine-applicable

#![feature(register_tool)]
#![allow(dead_code, enforce_inherent_impl_item_order, enforce_type_dependency_order)]
#![register_tool(rlib_lint)]
#![warn(enforce_module_declaration_order)]

fn run() -> Value {
    make()
}

fn make() -> Value {
    Value
}

struct Value;

fn main() {}
