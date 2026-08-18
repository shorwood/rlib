// run-rustfix
// rustfix-only-machine-applicable

#![feature(register_tool)]
#![allow(dead_code, rlib::misordered_module_declarations, rlib::misordered_type_declarations)]
#![register_tool(rlib_lint)]
#![warn(rlib::misordered_inherent_impl_items)]

struct Widget;

impl Widget {
    fn render(&self) {
        self.prepare();
    }

    const LIMIT: usize = 1;

    fn prepare(&self) {}
}

fn main() {}
