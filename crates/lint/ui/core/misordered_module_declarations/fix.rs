// run-rustfix
// rustfix-only-machine-applicable

#![feature(register_tool)]
#![allow(dead_code, misordered_inherent_impl_items, needless_function_wrappers, misordered_type_declarations)]
#![register_tool(rlib_lint)]
#![warn(misordered_module_declarations)]

fn run() -> Value {
    make()
}

fn make() -> Value {
    Value
}

struct Value;

fn main() {}
