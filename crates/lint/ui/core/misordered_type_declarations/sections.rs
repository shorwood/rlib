#![allow(dead_code, misordered_inherent_impl_items, misordered_module_declarations)]
#![warn(misordered_type_declarations)]

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
