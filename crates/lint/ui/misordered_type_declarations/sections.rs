#![allow(dead_code, misordered_inherent_impl_items, misordered_module_declarations)]
#![warn(misordered_type_declarations)]

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
