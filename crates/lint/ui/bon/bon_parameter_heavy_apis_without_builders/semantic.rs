#![allow(dead_code, rlib::misordered_module_declarations, unknown_lints)]

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

pub struct Renderer;

impl Renderer {
    pub fn configure(&self, path: PathBuf, cache: bool, color: bool, width: u32, height: u32) {
        let _ = (path, cache, color, width, height);
    }
}

pub fn many_distinct(
    first: PathBuf,
    second: String,
    third: u8,
    fourth: u16,
    fifth: u32,
    sixth: u64,
    seventh: usize,
) {
    let _ = (first, second, third, fourth, fifth, sixth, seventh);
}

mod internal {
    use super::PathBuf;

    pub fn syntactically_public(path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {
        let _ = (path, width, height, cache, color);
    }
}

pub trait RenderContract {
    fn render(&self, path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {
        let _ = (path, width, height, cache, color);
    }
}

fn main() {}
