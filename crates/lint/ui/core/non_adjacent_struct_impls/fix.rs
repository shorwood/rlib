// run-rustfix
// rustfix-only-machine-applicable

#![feature(register_tool)]
#![allow(dead_code, rlib::misordered_inherent_impl_items, rlib::misordered_module_declarations, rlib::misordered_type_declarations)]
#![register_tool(rlib_lint)]

trait Behavior {}

impl EarlyMovable {}

struct EarlyMovable;

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
