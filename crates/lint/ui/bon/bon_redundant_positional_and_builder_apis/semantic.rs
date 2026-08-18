#![allow(
    rlib::bon_undocumented_builder_members,
    dead_code,
    rlib::misordered_module_declarations,
    unknown_lints
)]

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

#[bon::builder]
pub fn converter_words_are_not_positional(
    #[builder(with = |value: PathBuf| { let _ = "start_fn"; value })] path: PathBuf,
    #[builder(with = |value: u32| { let _ = "finish_fn"; value })] width: u32,
    #[builder(with = |value: u32| { let _ = "start_fn"; value })] height: u32,
    cache: bool,
    color: bool,
) {
    let _ = (path, width, height, cache, color);
}

mod internal {
    use super::PathBuf;

    #[bon::builder]
    pub fn unreachable(
        #[builder(start_fn)] path: PathBuf,
        #[builder(start_fn)] width: u32,
        #[builder(start_fn)] height: u32,
        #[builder(finish_fn)] cache: bool,
        color: bool,
    ) {
        let _ = (path, width, height, cache, color);
    }
}

fn main() {}
