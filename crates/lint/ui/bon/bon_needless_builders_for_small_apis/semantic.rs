#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::path::PathBuf;

#[bon::builder]
fn checksum(path: PathBuf, retries: u32) {
    let _ = (path, retries);
}

#[bon::builder]
pub fn public_checksum(path: PathBuf, retries: u32) {
    let _ = (path, retries);
}

#[bon::builder]
fn optional_checksum(path: PathBuf, format: Option<String>) {
    let _ = (path, format);
}

fn main() {}
