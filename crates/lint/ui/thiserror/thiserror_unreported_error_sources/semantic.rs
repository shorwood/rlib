#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
#[error("transport failed")]
struct TransportError;

#[derive(Debug, thiserror::Error)]
#[error("request failed: {cause}")]
struct RequestError {
    cause: TransportError,
}

#[derive(Debug, thiserror::Error)]
#[error("connected request failed")]
struct ChainedRequestError {
    #[source]
    cause: TransportError,
}

fn main() {}
