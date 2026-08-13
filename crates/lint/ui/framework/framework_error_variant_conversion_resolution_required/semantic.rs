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

fn main() {}
