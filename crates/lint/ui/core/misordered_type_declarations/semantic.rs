#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]
#![allow(misordered_inherent_impl_items)]
#![allow(misordered_module_declarations)]
#![warn(misordered_type_declarations)]

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

struct Left(Option<Box<Right>>);
struct Right(Option<Box<Left>>);

fn main() {}
