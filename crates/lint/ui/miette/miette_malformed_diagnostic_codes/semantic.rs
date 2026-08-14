#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad")]
#[diagnostic(code(Config::InvalidValue))]
struct Bad;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("good")]
#[diagnostic(code(config::invalid_value))]
struct GoodPath;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("good")]
#[diagnostic(code(E1042))]
struct GoodNumber;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("good")]
#[diagnostic(code("storage2::invalid_value3"))]
struct GoodStringPath;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad")]
#[diagnostic(code(config::invalid__value))]
struct RepeatedSeparator;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad")]
#[diagnostic(code("config::invalid_"))]
struct TrailingSeparator;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum VariantCodes {
    #[error("bad")]
    #[diagnostic(code(E10A))]
    MixedNumber,
    #[error("good")]
    #[diagnostic(code(network::unavailable))]
    StablePath,
}

fn main() {}
