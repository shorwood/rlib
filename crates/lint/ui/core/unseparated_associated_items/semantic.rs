#![allow(dead_code, rlib::non_adjacent_struct_impls)]
#![warn(rlib::unseparated_associated_items)]

trait Compact {
    type Input;
    // This comment belongs to the following operation, but its boundary remains dense.
    fn evaluate(input: Self::Input);
}

struct Service;

impl Service {
    fn first(&self) {}
    fn second(&self) {}

    fn already_separated(&self) {}
}

#[allow(rlib::unseparated_associated_items)]
impl Service {
    fn locally_allowed_first(&self) {}
    fn locally_allowed_second(&self) {}
}

macro_rules! generated_methods {
    () => {
        fn generated_first(&self) {}
        fn generated_second(&self) {}
    };
}

impl Service {
    generated_methods!();
}

fn main() {}
