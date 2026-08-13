#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::path::PathBuf;

#[bon::builder]
pub fn render(
    #[builder(start_fn)] path: PathBuf,
    #[builder(start_fn)] width: u32,
    #[builder(start_fn)] height: u32,
    #[builder(finish_fn)] cache: bool,
    color: bool,
) {
    let _ = (path, width, height, cache, color);
}

#[bon::builder]
pub fn named(
    #[builder(start_fn)] path: PathBuf,
    width: u32,
    height: u32,
    cache: bool,
    color: bool,
) {
    let _ = (path, width, height, cache, color);
}

fn main() {}
