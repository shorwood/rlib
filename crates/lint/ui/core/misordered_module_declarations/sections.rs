#![allow(dead_code, misordered_inherent_impl_items, misordered_type_declarations)]
#![warn(misordered_module_declarations)]

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
