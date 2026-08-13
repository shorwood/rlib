#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("unfocused")]
struct Unfocused {
    #[source_code]
    input: miette::NamedSource<String>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("focused")]
struct Focused {
    #[source_code]
    input: miette::NamedSource<String>,
    #[label]
    span: miette::SourceSpan,
}

fn main() {}
