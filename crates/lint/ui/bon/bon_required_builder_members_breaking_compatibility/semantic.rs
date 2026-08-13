#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[derive(bon::Builder)]
pub struct Request {
    host: String,
    port: u16,
    /// An absent label disables labeling.
    label: Option<String>,
    /// Number of retries; defaults to three.
    #[builder(default = 3)]
    retries: u32,
}

fn main() {}
