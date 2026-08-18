#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(
    clippy::missing_const_for_fn,
    dead_code,
    rlib::misordered_inherent_impl_items
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

    fn cardinality() -> usize {
        3
    }

    const RETRY_LIMIT: usize = 3;

    const COUNT_U8: u8 = 3;

    fn version() -> usize {
        3
    }

    fn generic_count<T>() -> usize {
        let _marker = std::marker::PhantomData::<T>;
        3
    }

    const WRONG_COUNT: usize = 2;

    fn contextual_count(_include_complete: bool) -> usize {
        3
    }
}

#[allow(rlib::strum_manual_enum_counts)]
impl Stage {
    const SUPPRESSED_COUNT: usize = 3;
}

fn main() {}
