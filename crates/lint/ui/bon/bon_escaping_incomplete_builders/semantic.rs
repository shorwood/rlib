#![allow(
    dead_code,
    misordered_module_declarations,
    needless_function_wrappers,
    unknown_lints
)]

#[derive(bon::Builder)]
pub struct Request {
    host: String,
}

fn base_request() -> RequestBuilder {
    Request::builder()
}

struct Pending {
    builder: RequestBuilder,
}

fn complete() -> Request {
    Request::builder().host(String::new()).build()
}

fn main() {}
