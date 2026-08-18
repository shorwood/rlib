#![allow(dead_code, rlib::misordered_inherent_impl_items, rlib::misordered_module_declarations)]
#![warn(rlib::misordered_type_declarations)]

mod authored {
    // -----------------------------------------------------------------------------
    // Consumer: Type ordering fixture
    // -----------------------------------------------------------------------------

    struct Consumer(Dependency);

    // -----------------------------------------------------------------------------
    // Dependency: Type ordering fixture
    // -----------------------------------------------------------------------------

    struct Dependency;
}

fn main() {}
