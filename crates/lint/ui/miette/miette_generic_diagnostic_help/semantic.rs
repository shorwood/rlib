#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("unavailable")]
#[diagnostic(help("try again"))]
struct Generic;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("missing configuration")]
#[diagnostic(help("set `config_path` to an existing readable file"))]
struct Actionable;

fn main() {}
