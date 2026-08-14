#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("I/O failed")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Format(std::fmt::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("I/O failed")]
pub struct OpaqueApiError(#[source] std::io::Error);

mod internal {
    #[derive(Debug, thiserror::Error)]
    pub enum InternalError {
        #[error(transparent)]
        Io(std::io::Error),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("local source")]
pub struct LocalSource;

#[derive(Debug, thiserror::Error)]
pub enum StableError {
    #[error("wrapped")]
    Wrapped { source: LocalSource },
}

fn main() {}
