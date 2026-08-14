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

mod shadowed_option {
    use super::{Error, fmt};

    #[derive(Debug)]
    struct ShadowedSource {
        source: std::io::Error,
    }

    impl fmt::Display for ShadowedSource {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("shadowed")
        }
    }

    impl Error for ShadowedSource {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.source)
        }
    }

    #[allow(non_snake_case)]
    fn Some<'a>(_: &'a (dyn Error + 'static)) -> Option<&'a (dyn Error + 'static)> {
        None
    }
}

fn main() {}
