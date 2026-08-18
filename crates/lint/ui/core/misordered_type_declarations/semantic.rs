#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]
#![allow(rlib::misordered_inherent_impl_items)]
#![allow(rlib::misordered_module_declarations)]
#![warn(rlib::misordered_type_declarations)]

// -----------------------------------------------------------------------------
// Fixture: Type ordering fixture
// -----------------------------------------------------------------------------

struct UsesAlias {
    value: Alias,
}

type Alias = Option<Box<Leaf>>;

struct Leaf;

trait UsesBound<T: Bound> {
    fn make(value: Leaf) -> Alias;
}

trait Bound {}

type Generic<T = GenericDependency> = T;

struct GenericDependency;

struct Left(Option<Box<Right>>);

struct Right(Option<Box<Left>>);

fn main() {}
