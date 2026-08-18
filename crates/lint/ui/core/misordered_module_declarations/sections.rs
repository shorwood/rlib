#![allow(dead_code, rlib::misordered_inherent_impl_items, rlib::misordered_type_declarations)]
#![warn(rlib::misordered_module_declarations)]

mod authored {
    // -----------------------------------------------------------------------------
    // Consumer: Declaration ordering fixture
    // -----------------------------------------------------------------------------

    struct Consumer(Dependency);

    // -----------------------------------------------------------------------------
    // Dependency: Declaration ordering fixture
    // -----------------------------------------------------------------------------

    struct Dependency;
}

fn main() {}
