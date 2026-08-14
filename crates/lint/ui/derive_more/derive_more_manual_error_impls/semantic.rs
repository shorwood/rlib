#![allow(dead_code, unknown_lints)]

use std::error::Error;
use std::fmt;

#[derive(Debug)]
struct Marker;

impl fmt::Display for Marker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("marker")
    }
}

impl Error for Marker {}

#[derive(Debug)]
struct SuppressedSource {
    source: std::io::Error,
}

impl fmt::Display for SuppressedSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("source intentionally suppressed")
    }
}

// False-positive boundary: derive_more would expose `source`, unlike this empty impl.
impl Error for SuppressedSource {}

#[derive(Debug)]
struct SuppressedTuple(std::io::Error);

impl fmt::Display for SuppressedTuple {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("tuple source intentionally suppressed")
    }
}

// False-positive boundary: derive_more treats a sole tuple field as the source.
impl Error for SuppressedTuple {}

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

mod shadowed_option {
    use super::{Error, fmt};

    #[derive(Debug)]
    pub struct Shadowed {
        source: std::io::Error,
    }

    impl fmt::Display for Shadowed {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("shadowed")
        }
    }

    impl Error for Shadowed {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            // False-positive boundary: this is not the standard `Option::Some` constructor.
            Some(&self.source)
        }
    }

    #[allow(non_snake_case)]
    fn Some(_source: &std::io::Error) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[derive(Debug)]
struct ContextError;

impl fmt::Display for ContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("context")
    }
}

impl Error for ContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

fn main() {}
