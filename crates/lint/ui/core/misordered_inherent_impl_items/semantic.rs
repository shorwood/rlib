#![feature(inherent_associated_types, register_tool)]
#![allow(dead_code, incomplete_features)]
#![register_tool(rlib_lint)]
#![allow(rlib::misordered_module_declarations)]
#![allow(rlib::non_adjacent_struct_impls)]
#![allow(rlib::misordered_type_declarations)]
#![warn(rlib::misordered_inherent_impl_items)]

struct Widget;

impl Widget {
    fn render(&self) {
        self.prepare();
    }

    fn helper() {}

    pub fn new() -> Option<Self> {
        Self::helper();
        Some(Self)
    }

    const LIMIT: usize = 4;

    type Output = Self;

    fn prepare(&self) {}
}

struct NamedWidget;

impl NamedWidget {
    fn helper() {}

    fn new() -> NamedWidget {
        NamedWidget
    }
}

trait External {
    fn later(&self);
    fn earlier(&self);
}

impl External for Widget {
    fn later(&self) {
        self.earlier();
    }

    fn earlier(&self) {}
}

impl Widget {
    fn separate(&self) {
        Self::helper();
    }
}

fn main() {}
