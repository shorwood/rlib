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

fn main() {}
