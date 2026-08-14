#![allow(dead_code, unknown_lints)]

#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("request failed")]
struct RequestError {
    source: std::io::Error,
    retry_error: std::io::Error,
}

#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("explicit request failure")]
struct ExplicitRequestError {
    #[error(source)]
    cause: std::io::Error,
    #[error(not(source))]
    retry_error: std::io::Error,
}

#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("partially classified request failure")]
struct PartiallyClassifiedError {
    source: std::io::Error,
    #[error(not(source))]
    context_error: std::io::Error,
    retry_cause: std::io::Error,
}

#[derive(Debug, derive_more::Display, derive_more::Error)]
enum TransportError {
    #[display("request failed")]
    Request {
        source: std::io::Error,
        parse_error: std::io::Error,
    },
    #[display("explicit failure")]
    Explicit {
        #[error(source)]
        cause: std::io::Error,
        other_error: std::io::Error,
    },
}

fn main() {}
