// run-rustfix
// rustfix-only-machine-applicable
// edition:2024

#![feature(register_tool)]
#![allow(dead_code, misordered_inherent_impl_items, misordered_module_declarations, misordered_type_declarations)]
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
