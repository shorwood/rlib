#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[bon::builder]
fn request(#[builder(into)] url: String, header: String) {
    let _ = (url, header);
}

#[bon::builder]
fn coherent(#[builder(into)] url: String, #[builder(into)] header: String) {
    let _ = (url, header);
}

#[bon::builder]
fn distinct(#[builder(into)] url: String, retries: u32) {
    let _ = (url, retries);
}

fn main() {}
