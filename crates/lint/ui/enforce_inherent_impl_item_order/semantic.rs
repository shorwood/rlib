#![feature(inherent_associated_types, register_tool)]
#![allow(dead_code, incomplete_features)]
#![register_tool(rlib_lint)]
#![allow(enforce_module_declaration_order)]
#![allow(enforce_struct_impl_definition_order)]
#![allow(enforce_type_dependency_order)]
#![warn(enforce_inherent_impl_item_order)]

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
