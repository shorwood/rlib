// run-rustfix
// rustfix-only-machine-applicable
// normalize-stderr-test: "(?m) +$" -> ""

#![feature(register_tool)]
#![allow(dead_code, misordered_inherent_impl_items, misordered_module_declarations)]
#![register_tool(rlib_lint)]
#![warn(misordered_type_declarations)]

// -----------------------------------------------------------------------------
// Fixture: Type ordering fixture
// -----------------------------------------------------------------------------

struct Consumer(Dependency);

impl Consumer {}

struct Dependency;

fn main() {}
