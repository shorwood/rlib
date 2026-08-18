#![feature(register_tool, rustc_private)]
#![register_tool(rlib)]
#![warn(unused_extern_crates)]
mod linting;
mod config;
mod utils;
mod rules;
mod registration;
