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
pub(crate) use defining::CrateItem as RootCrateItem;
pub(in crate) use defining::RestrictedItem as RootRestrictedItem;
pub use defining::Choice::*;
pub use std::fmt::Debug as ExternalDebug;

mod nested {
    pub(super) use crate::defining::ParentItem;
    pub(crate) use crate::defining::CrateItem;
    pub(in crate) use crate::defining::RestrictedItem;

    use crate::defining::PublicItem;
    pub(self) use crate::defining::CrateItem as SelfCrateItem;
    pub(in self) use crate::defining::RestrictedItem as SelfRestrictedItem;
    pub(in crate::nested) use crate::defining::Choice;
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
