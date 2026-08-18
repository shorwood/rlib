
#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]

#[path = "auxiliary/directory_parent.rs"]
mod directory_parent;
#[path = "auxiliary/explicit_parent.rs"]
mod explicit_parent;
#[path = "auxiliary/mixed_parent.rs"]
mod mixed_parent;
#[path = "auxiliary/conventional/mod.rs"]
mod conventional;
#[path = "auxiliary/standalone_parent.rs"]
mod standalone_parent;
#[path = "auxiliary/rust_child_parent.rs"]
mod rust_child_parent;

mod inline_module {
    struct Local;
}

fn main() {}
