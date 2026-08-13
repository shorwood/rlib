#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ConfigError {
    #[error("missing")]
    #[diagnostic(code(config::missing))]
    Missing,
    #[error("invalid")]
    Invalid,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("internal")]
struct Internal;

fn main() {}
