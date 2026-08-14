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

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum TransparentMember {
    #[error("coded")]
    #[diagnostic(code(network::coded))]
    Coded,
    #[error(transparent)]
    #[diagnostic(transparent)]
    Forwarded(
        #[from]
        #[diagnostic_source]
        Internal,
    ),
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum MultipleMissing {
    #[error("coded")]
    #[diagnostic(code("storage::coded"))]
    Coded,
    #[error("first missing")]
    FirstMissing,
    #[error("second missing")]
    SecondMissing,
}

fn main() {}
