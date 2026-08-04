#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]
#![allow(enforce_inherent_impl_item_order)]
#![allow(enforce_module_declaration_order)]
#![warn(enforce_type_dependency_order)]

struct UsesAlias {
    value: Alias,
}

type Alias = Option<Box<Leaf>>;

struct Leaf;

trait UsesBound<T: Bound> {
    fn make(value: Leaf) -> Alias;
}

trait Bound {}

struct Left(Option<Box<Right>>);
struct Right(Option<Box<Left>>);

fn main() {}
