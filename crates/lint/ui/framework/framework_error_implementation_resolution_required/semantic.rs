#![allow(dead_code, unknown_lints)]

use std::error::Error;
use std::fmt;

#[derive(Debug)]
struct LoadError {
    source: std::io::Error,
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("load failed")
    }
}

impl Error for LoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

fn main() {}
