#![feature(register_tool)]
#![allow(dead_code, misordered_inherent_impl_items, misordered_module_declarations)]
#![register_tool(rlib_lint)]
#![warn(misordered_type_declarations)]

mod attributed {
    // -----------------------------------------------------------------------------
    // Attributed: Type ordering fixture
    // -----------------------------------------------------------------------------

    #[repr(C)]
    struct User(Dependency);

    struct Dependency;
}

mod commented {
    // -----------------------------------------------------------------------------
    // Commented: Type ordering fixture
    // -----------------------------------------------------------------------------

    struct User(Dependency);
    // This comment has deliberately ambiguous ownership.

    struct Dependency;
}

mod macro_crossing {
    // -----------------------------------------------------------------------------
    // MacroCrossing: Type ordering fixture
    // -----------------------------------------------------------------------------

    struct User(Dependency);
    macro_rules! boundary {
        () => {};
    }
    struct Dependency;
}

macro_rules! generated_type {
    () => {
        struct Generated(GeneratedDependency);
    };
}

mod expanded {
    // -----------------------------------------------------------------------------
    // Expanded: Type ordering fixture
    // -----------------------------------------------------------------------------

    generated_type!();
    struct GeneratedDependency;
}

fn main() {}
