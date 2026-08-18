#![allow(
    rlib::bon_escaping_incomplete_builders,
    dead_code,
    rlib::misordered_module_declarations,
    rlib::needless_function_wrappers,
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

pub struct PublicState {
    pub pending: Option<RequestBuilder>,
}

struct PrivateState {
    pub pending: Option<RequestBuilder>,
}

pub struct Service;

impl Service {
    pub fn raw_pending(builder: *const RequestBuilder) {
        let _ = builder;
    }
}

mod internal {
    use super::{Request, RequestBuilder};

    pub fn hidden_partial() -> RequestBuilder {
        Request::builder()
    }
}

fn main() {}
