#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ConfigError {
    #[error("missing value")]
    #[diagnostic(code(config::invalid))]
    Missing,
    #[error("invalid value")]
    #[diagnostic(code(config::invalid))]
    Invalid,
    #[error("unavailable")]
    #[diagnostic(code(config::unavailable))]
    Unavailable,
}

fn main() {}
