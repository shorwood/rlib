#![feature(register_tool)]
#![allow(dead_code, enforce_inherent_impl_item_order, enforce_module_declaration_order)]
#![register_tool(rlib_lint)]
#![warn(enforce_type_dependency_order)]

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
