// run-rustfix
// rustfix-only-machine-applicable
// normalize-stderr-test: "(?m) +$" -> ""

#![feature(register_tool)]
#![allow(dead_code, rlib::misordered_inherent_impl_items, rlib::misordered_module_declarations)]
#![register_tool(rlib_lint)]
#![warn(rlib::misordered_type_declarations)]

// -----------------------------------------------------------------------------
// Fixture: Type ordering fixture
// -----------------------------------------------------------------------------

struct Consumer(Dependency);

impl Consumer {}

struct Dependency;

fn main() {}
