#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("parse failed")]
struct ParseError;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("load failed")]
struct Plain {
    #[source]
    source: ParseError,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("load failed")]
struct Forwarded {
    #[source]
    #[diagnostic_source]
    source: ParseError,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("boxed")]
struct Boxed {
    #[source]
    nested: Box<ParseError>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("implicit source name")]
struct ImplicitSource {
    source: Box<ParseError>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("converted")]
struct Converted {
    #[from]
    nested: ParseError,
}

fn main() {}
