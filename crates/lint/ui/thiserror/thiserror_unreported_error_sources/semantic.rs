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

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct TransparentRequestError {
    cause: TransportError,
}

#[derive(Debug, thiserror::Error)]
enum RoutedError {
    #[error("read failed")]
    Read { read_error: TransportError },
    #[error("write failed")]
    Write { write_error: TransportError },
    #[error("connected")]
    Connected {
        #[source]
        cause: TransportError,
    },
    #[error(transparent)]
    Transparent { cause: TransportError },
}

fn main() {}
