#![allow(dead_code, unused_extern_crates, unused_imports)]
#![warn(non_defining_module_reexports)]

mod defining {
    pub struct PublicItem;
    pub struct CrateItem;
    pub struct RestrictedItem;
    pub struct ParentItem;

    pub enum Choice {
        First,
    }
}

pub use defining::PublicItem;
pub(crate) use defining::CrateItem;
pub(in crate) use defining::RestrictedItem;
pub use defining::Choice::*;
pub use std::fmt::Debug as ExternalDebug;

mod nested {
    pub(super) use crate::defining::ParentItem;

    use crate::defining::PublicItem;
    pub(self) use crate::defining::CrateItem;
}

pub extern crate core as exported_core;

macro_rules! generated_reexport {
    () => {
        pub use defining::CrateItem as GeneratedItem;
    };
}

generated_reexport!();

#[allow(non_defining_module_reexports)]
pub use defining::PublicItem as ExplicitlyAllowed;

fn main() {}
