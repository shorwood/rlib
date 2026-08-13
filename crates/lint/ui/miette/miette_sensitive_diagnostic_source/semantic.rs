#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("invalid request")]
struct Exposed {
    #[source_code]
    request_body: miette::NamedSource<String>,
    #[label]
    span: miette::SourceSpan,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("invalid request")]
struct Redacted {
    #[source_code]
    redacted_request: miette::NamedSource<String>,
    #[label]
    span: miette::SourceSpan,
}

fn main() {}
