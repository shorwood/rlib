#![feature(rustc_private)]
#![warn(unused_extern_crates)]
pub(crate) mod utils;
pub(crate) mod rules;
mod registration;

pub use registration::register_lints;
