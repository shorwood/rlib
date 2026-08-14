#![allow(dead_code, unknown_lints)]

use std::io;

#[derive(Debug, thiserror::Error)]
enum LoadError {
    #[error("I/O failed")]
    Io(#[source] io::Error),
}

impl From<io::Error> for LoadError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

#[derive(Debug, thiserror::Error)]
enum NamedLoadError {
    #[error("I/O failed")]
    Io { source: io::Error },
}

impl From<io::Error> for NamedLoadError {
    fn from(source: io::Error) -> Self {
        Self::Io { source }
    }
}

#[derive(Debug, thiserror::Error)]
enum ContextOnlyError {
    #[error("I/O failed")]
    Io(io::Error),
}

impl From<io::Error> for ContextOnlyError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

fn main() {}
