#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]
#![allow(enforce_inherent_impl_item_order)]
#![allow(enforce_type_dependency_order)]
#![warn(enforce_module_declaration_order)]

use std::mem::size_of;

fn run(callback: fn(Anchor) -> usize) -> usize {
    (unsafe { foreign_value() }) + callback(Anchor) + child::VALUE + LIMIT
}

const LIMIT: usize = 1;

struct Anchor;

impl Anchor {
    fn size(self) -> usize {
        size_of::<Self>()
    }
}

mod child {
    pub const VALUE: usize = 2;
}

unsafe extern "C" {
    fn foreign_value() -> usize;
}

fn main() {
    let _ = run(Anchor::size);
}
