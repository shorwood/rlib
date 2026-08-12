use std::fmt;
pub(self) use std::io;
pub(in self) use std::mem;

mod inline_child {
    // The inline module receives one warning as a whole. Its children must not receive duplicates.
    fn nested_implementation() {}
    struct NestedType;
}

fn implementation() {}
struct ImplementedHere;

macro_rules! make_item {
    () => {
        struct GeneratedItem;
    };
}

make_item!();
include!("included.rs");

#[allow(invalid_barrel_file_items)]
fn explicitly_allowed() {}
