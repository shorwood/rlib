#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
#[error("cleanup failed")]
struct CleanupError;

#[derive(Debug, thiserror::Error)]
#[error("read failed")]
struct ReadError;

#[derive(Debug, thiserror::Error)]
#[error("operation failed")]
struct ClassifiedError {
    #[source]
    source: ReadError,
    suppressed_cleanup: CleanupError,
}

#[derive(Debug, thiserror::Error)]
#[error("operation failed")]
struct OperationError {
    #[source]
    primary_error: ReadError,
    secondary_error: CleanupError,
}

fn main() {}
