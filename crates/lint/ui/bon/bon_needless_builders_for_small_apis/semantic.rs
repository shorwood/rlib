#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::path::PathBuf;

use bon::bon;

#[bon::builder]
fn checksum(path: PathBuf, retries: u32) {
    let _ = (path, retries);
}

#[bon::builder]
fn one_argument(path: PathBuf) {
    let _ = path;
}

#[bon::builder]
fn documented_parameter(#[allow(unused_variables)] path: PathBuf, retries: u32) {
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

#[bon::builder]
fn qualified_optional_checksum(path: PathBuf, format: std::option::Option<String>) {
    let _ = (path, format);
}

#[bon::builder]
fn repeated_types(left: u32, right: u32) {
    let _ = (left, right);
}

#[bon::builder]
fn configured_member(path: PathBuf, #[builder(default)] retries: u32) {
    let _ = (path, retries);
}

struct Checksums;

#[bon]
impl Checksums {
    #[builder]
    fn associated(path: PathBuf, retries: u32) {
        let _ = (path, retries);
    }

    #[builder]
    fn method(&self, path: PathBuf) {
        let _ = path;
    }
}

fn main() {}
