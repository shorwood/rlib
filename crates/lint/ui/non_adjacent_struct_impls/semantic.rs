// aux-build: external_macro.rs
// edition:2024

#![feature(register_tool)]
#![allow(dead_code, misordered_inherent_impl_items, misordered_module_declarations, misordered_type_declarations)]
#![register_tool(rlib_lint)]

extern crate external_macro;

use external_macro::external_layout;

trait Behavior {}

// A data-only struct does not need an empty impl block.
struct DataOnly;

// Every direct impl may form one group, with comments between blocks.
struct Ordered;

impl Ordered {}

// Trait behavior remains part of the same group.
impl Behavior for Ordered {}

// Tuple, unit, generic, and alias spellings all resolve to their struct.
struct Tuple(u8);
impl Tuple {}

struct Generic<T>(T);
impl<T> Generic<T> {}
impl Generic<u8> {}

type Alias = Aliased;
struct Aliased;
impl Alias {}

// A different definition between the struct and its impl is a violation.
struct Delayed;
fn delayed_separator() {}
impl Delayed {}

// An impl before its struct is also outside the required group.
impl Early {}
struct Early;

// All impl blocks must stay together after the struct.
struct Split;
impl Split {}
fn split_separator() {}
impl Behavior for Split {}

// Reference self-types are not direct impls for the struct.
trait ReferenceBehavior {}
struct Referenced;
impl ReferenceBehavior for &Referenced {}

// Cross-module placement belongs to the separate colocation rule.
mod owner {
    pub(super) struct Remote;
}

mod implementations {
    impl super::owner::Remote {}
}

// Local macro output is controlled by this crate and remains subject to the rule.
macro_rules! local_layout {
    () => {
        struct FromLocalMacro;
        fn local_separator() {}
        impl FromLocalMacro {}
    };
}

local_layout!();

// External macro output is outside the agent's control and is ignored.
external_layout!();

fn main() {}
