#![allow(dead_code, misordered_inherent_impl_items, misordered_type_declarations)]
#![warn(misordered_module_declarations)]

mod authored {
    // -----------------------------------------------------------------------------
    // Consumer
    // -----------------------------------------------------------------------------

    struct Consumer(Dependency);

    // -----------------------------------------------------------------------------
    // Dependency
    // -----------------------------------------------------------------------------

    struct Dependency;
}

fn main() {}
