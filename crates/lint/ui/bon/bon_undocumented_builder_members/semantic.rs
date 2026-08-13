#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[bon::builder]
pub fn connect(#[builder(default = 30)] timeout_seconds: u64, host: String) {
    let _ = (timeout_seconds, host);
}

#[bon::builder]
pub fn documented(
    /// An absent label disables display labeling.
    label: Option<String>,
) {
    let _ = label;
}

#[bon::builder]
fn private_api(#[builder(into)] host: String) {
    let _ = host;
}

#[derive(bon::Builder)]
pub struct Request {
    #[builder(into)]
    destination: String,
    payload: Vec<u8>,
}

fn main() {}
