#![feature(register_tool)]
#![allow(dead_code, misordered_inherent_impl_items, misordered_module_declarations)]
#![register_tool(rlib_lint)]
#![warn(misordered_type_declarations)]

mod attributed {
    #[repr(C)]
    struct User(Dependency);
    struct Dependency;
}

mod commented {
    struct User(Dependency);
    // This comment has deliberately ambiguous ownership.
    struct Dependency;
}

mod macro_crossing {
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
    generated_type!();
    struct GeneratedDependency;
}

fn main() {}
