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

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("primary storage failure")]
#[diagnostic(code(storage::shared))]
struct PrimaryStorage;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("fallback storage failure")]
#[diagnostic(code("storage::shared"))]
struct FallbackStorage;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("unique storage failure")]
#[diagnostic(code(storage::unique))]
struct UniqueStorage;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[diagnostic(code(protocol::shared))]
enum ProtocolError {
    #[error("disconnected")]
    Disconnected,
    #[error("timed out")]
    TimedOut,
}

fn main() {}
