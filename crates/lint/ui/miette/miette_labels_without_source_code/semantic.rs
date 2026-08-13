#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad span")]
struct MissingSource {
    #[label]
    span: miette::SourceSpan,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad span")]
struct Complete {
    #[source_code]
    input: miette::NamedSource<String>,
    #[label]
    span: miette::SourceSpan,
}

fn main() {}
