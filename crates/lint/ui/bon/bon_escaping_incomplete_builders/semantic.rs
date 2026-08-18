#![allow(
    dead_code,
    rlib::misordered_module_declarations,
    rlib::needless_function_wrappers,
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

type PendingAlias = RequestBuilder;

fn optional_base_request() -> Option<PendingAlias> {
    Some(Request::builder())
}

struct NestedPending {
    builders: (Option<RequestBuilder>, *const RequestBuilder),
}

fn complete() -> Request {
    Request::builder().host(String::new()).build()
}

fn main() {}
