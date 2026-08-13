#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("deprecated input")]
#[diagnostic(severity(Warning))]
struct DeprecatedInput;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("invalid input")]
#[diagnostic(severity(Error))]
struct InvalidInput;

fn validate() -> Result<(), DeprecatedInput> {
    Err(DeprecatedInput)
}

fn reject() -> Result<(), InvalidInput> {
    Err(InvalidInput)
}

fn main() {}
