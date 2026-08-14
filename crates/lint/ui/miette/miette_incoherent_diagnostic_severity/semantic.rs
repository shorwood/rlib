#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("deprecated input")]
#[diagnostic(severity(Warning))]
struct DeprecatedInput;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("invalid input")]
#[diagnostic(severity(Error))]
struct InvalidInput;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("migration advice")]
#[diagnostic(severity(Advice))]
struct MigrationAdvice;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[diagnostic(severity(Warning))]
enum ProtocolWarning {
    #[error("legacy protocol")]
    Legacy,
    #[error("deprecated protocol")]
    Deprecated,
}

struct Validator;

impl Validator {
    fn validate_boxed(&self) -> Result<(), Box<DeprecatedInput>> {
        Err(Box::new(DeprecatedInput))
    }
}

fn validate() -> Result<(), DeprecatedInput> {
    Err(DeprecatedInput)
}

fn reject() -> Result<(), InvalidInput> {
    Err(InvalidInput)
}

fn advise() -> Result<(), std::sync::Arc<MigrationAdvice>> {
    Err(std::sync::Arc::new(MigrationAdvice))
}

fn warn_protocol() -> Result<(), ProtocolWarning> {
    Err(ProtocolWarning::Legacy)
}

fn main() {}
