#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("ambiguous")]
struct Unfocused {
    #[source_code]
    input: miette::NamedSource<String>,
    #[label]
    first: miette::SourceSpan,
    #[label]
    second: miette::SourceSpan,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("focused")]
struct Focused {
    #[source_code]
    input: miette::NamedSource<String>,
    #[label(primary)]
    first: miette::SourceSpan,
    #[label]
    second: miette::SourceSpan,
}

fn main() {}
