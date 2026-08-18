#![feature(register_tool)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]
#![allow(rlib::misordered_inherent_impl_items)]
#![allow(rlib::misordered_type_declarations)]
#![warn(rlib::misordered_module_declarations)]
#![allow(rlib::needless_function_wrappers)]

use std::mem::size_of;

mod deferred_impl_regression {
    #![allow(rlib::non_adjacent_struct_impls)]

    // -----------------------------------------------------------------------------
    // DeferredImplRegression: Declaration ordering fixture
    // -----------------------------------------------------------------------------

    struct Deferred;

    const SEPARATOR: usize = 0;

    impl Deferred {
        fn run() -> usize {
            helper()
        }
    }

    fn helper() -> usize {
        SEPARATOR
    }
}

mod local_macro_output {
    macro_rules! generated_declarations {
        () => {
            fn generated() -> Generated {
                Generated
            }

            struct Generated;
        };
    }

    // -----------------------------------------------------------------------------
    // GeneratedDeclarations: Generated declaration catalog
    // -----------------------------------------------------------------------------

    generated_declarations!();
}

// -----------------------------------------------------------------------------
// Fixture: Declaration ordering fixture
// -----------------------------------------------------------------------------

fn run(callback: fn(Anchor) -> usize) -> usize {
    (unsafe { foreign_value() }) + callback(Anchor) + child::VALUE + LIMIT
}

const LIMIT: usize = 1;

struct Anchor;

impl Anchor {
    fn size(self) -> usize {
        size_of::<Self>()
    }
}

mod child {
    pub const VALUE: usize = 2;
}

unsafe extern "C" {
    fn foreign_value() -> usize;
}

fn main() {
    let _ = run(Anchor::size);
}
