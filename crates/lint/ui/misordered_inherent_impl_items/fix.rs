// run-rustfix
// rustfix-only-machine-applicable

#![feature(register_tool)]
#![allow(dead_code, misordered_module_declarations, misordered_type_declarations)]
#![register_tool(rlib_lint)]
#![warn(misordered_inherent_impl_items)]

struct Widget;

impl Widget {
    fn render(&self) {
        self.prepare();
    }

    const LIMIT: usize = 1;

    fn prepare(&self) {}
}

fn main() {}
