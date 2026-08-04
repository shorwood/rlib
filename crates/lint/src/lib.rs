#![feature(rustc_private)]
#![warn(unused_extern_crates)]

mod registration;
mod rules;

pub use registration::register_lints;
