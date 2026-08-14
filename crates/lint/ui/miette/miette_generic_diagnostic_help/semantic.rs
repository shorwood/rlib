#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("unavailable")]
#[diagnostic(help("try again"))]
struct Generic;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("still unavailable")]
#[diagnostic(help("  TRY   AGAIN?!  "))]
struct DecoratedGeneric;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("support required")]
#[diagnostic(help("Contact support..."))]
struct PunctuatedGeneric;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("missing configuration")]
#[diagnostic(help("set `config_path` to an existing readable file"))]
struct Actionable;

fn main() {}
