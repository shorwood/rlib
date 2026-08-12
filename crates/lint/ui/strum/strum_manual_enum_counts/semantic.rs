#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(
    clippy::missing_const_for_fn,
    dead_code,
    misordered_inherent_impl_items
)]
#![register_tool(rlib_lint)]

enum Stage {
    Planned,
    Running,
    Complete,
}

impl Stage {
    const COUNT: usize = 3;

    pub fn variant_count() -> usize {
        3
    }

    const WRONG_COUNT: usize = 2;

    fn contextual_count(_include_complete: bool) -> usize {
        3
    }
}

#[allow(strum_manual_enum_counts)]
impl Stage {
    const SUPPRESSED_COUNT: usize = 3;
}

fn main() {}
