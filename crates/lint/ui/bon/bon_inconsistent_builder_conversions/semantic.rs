#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use bon::bon;

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

#[bon::builder]
fn custom_converter_is_incomparable(
    #[builder(into)] url: String,
    #[builder(with = |value: impl Into<String>| value.into())] header: String,
) {
    let _ = (url, header);
}

struct Client;

#[bon]
impl Client {
    #[builder]
    fn connect(#[builder(into)] host: String, label: String) -> Self {
        let _ = (host, label);
        Self
    }
}

#[derive(bon::Builder)]
struct RequestOptions {
    #[builder(into)]
    host: String,
    label: String,
}

fn main() {}
