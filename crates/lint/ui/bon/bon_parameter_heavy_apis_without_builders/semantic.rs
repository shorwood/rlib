#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::path::PathBuf;

pub fn render(path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {
    let _ = (path, width, height, cache, color);
}

#[bon::builder]
pub fn export(path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {
    let _ = (path, width, height, cache, color);
}

pub fn point(x: f64, y: f64, z: f64) {
    let _ = (x, y, z);
}

fn main() {}
