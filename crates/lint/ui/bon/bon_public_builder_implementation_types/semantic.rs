#![allow(
    bon_escaping_incomplete_builders,
    dead_code,
    misordered_module_declarations,
    needless_function_wrappers,
    unknown_lints
)]

#[derive(bon::Builder)]
pub struct Request {
    host: String,
}

pub fn partially_configured() -> RequestBuilder {
    Request::builder()
}

fn private_partial() -> RequestBuilder {
    Request::builder()
}

pub fn configured() -> Request {
    Request::builder().host(String::new()).build()
}

fn main() {}
