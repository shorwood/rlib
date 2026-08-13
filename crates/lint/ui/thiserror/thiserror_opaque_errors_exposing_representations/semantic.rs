#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("I/O failed")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("I/O failed")]
pub struct OpaqueApiError(#[source] std::io::Error);

fn main() {}
