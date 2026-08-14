#![warn(ad_hoc_error_interfaces)]
#![allow(dead_code, misordered_module_declarations, missing_section_dividers)]

use std::error::Error;
use std::fmt::{self, Display, Formatter};

struct ParseFailure {
    message: String,
}

fn parse_failure() -> Result<(), ParseFailure> {
    Err(ParseFailure { message: "invalid".to_owned() })
}

struct Notice {
    pub message: String,
}

fn notice() -> Result<(), Notice> {
    Err(Notice { message: "notice".to_owned() })
}

struct MessageOnly {
    message: String,
}

impl MessageOnly {
    fn message(&self) -> &str {
        &self.message
    }
}

fn message_only() -> Result<(), MessageOnly> {
    Err(MessageOnly { message: "message".to_owned() })
}

#[derive(Debug)]
struct InnerError;

impl Display for InnerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("inner")
    }
}

impl Error for InnerError {}

#[derive(Debug)]
struct CauseError {
    cause: InnerError,
}

impl Display for CauseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("cause")
    }
}

impl CauseError {
    fn cause(&self) -> &InnerError {
        &self.cause
    }
}

fn cause_error() -> Result<(), CauseError> {
    Err(CauseError { cause: InnerError })
}

#[derive(Debug)]
struct ExistingNoSource {
    cause: InnerError,
}

impl Display for ExistingNoSource {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("existing")
    }
}

impl Error for ExistingNoSource {}

impl ExistingNoSource {
    fn source(&self) -> &(dyn Error + 'static) {
        &self.cause
    }
}

fn existing_no_source() -> Result<(), ExistingNoSource> {
    Err(ExistingNoSource { cause: InnerError })
}

#[derive(Debug)]
struct CompleteError {
    cause: InnerError,
}

impl Display for CompleteError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("complete")
    }
}

impl Error for CompleteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

fn complete() -> Result<(), CompleteError> {
    Err(CompleteError { cause: InnerError })
}

struct NamedError;

fn name_only() -> Result<(), NamedError> {
    Err(NamedError)
}

struct Diagnostic {
    pub message: String,
}

fn diagnostic() -> Result<(), Diagnostic> {
    Err(Diagnostic { message: "data".to_owned() })
}

struct GenericError<T> {
    message: T,
}

fn generic_error() -> Result<(), GenericError<String>> {
    Err(GenericError { message: "generic".to_owned() })
}

static SHARED_INNER_ERROR: InnerError = InnerError;

#[derive(Debug)]
struct UnrelatedCauseError {
    is_marked: bool,
}

impl Display for UnrelatedCauseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("unrelated")
    }
}

impl Error for UnrelatedCauseError {}

impl UnrelatedCauseError {
    // False-positive boundary: touching `self` does not make a global error its predecessor.
    fn cause(&self) -> &InnerError {
        let _marker = self.is_marked;
        &SHARED_INNER_ERROR
    }
}

fn unrelated_cause() -> Result<(), UnrelatedCauseError> {
    Err(UnrelatedCauseError { is_marked: false })
}

#[derive(Debug)]
struct AliasedCauseError {
    cause: InnerError,
}

impl Display for AliasedCauseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("aliased")
    }
}

impl Error for AliasedCauseError {}

impl AliasedCauseError {
    // False-negative boundary: a receiver-rooted alias still exposes one predecessor.
    fn cause(&self) -> &InnerError {
        let cause = &self.cause;
        cause
    }
}

fn aliased_cause() -> Result<(), AliasedCauseError> {
    Err(AliasedCauseError { cause: InnerError })
}

#[derive(Debug)]
struct OptionalCauseError {
    cause: InnerError,
}

impl Display for OptionalCauseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("optional")
    }
}

impl Error for OptionalCauseError {}

impl OptionalCauseError {
    // False-negative boundary: an optional direct cause is the standard source shape.
    fn inner(&self) -> Option<&InnerError> {
        Some(&self.cause)
    }
}

fn optional_cause() -> Result<(), OptionalCauseError> {
    Err(OptionalCauseError { cause: InnerError })
}

fn main() {}
