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

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("collection without a cause")]
struct CollectionOnly {
    #[source_code]
    input: String,
    #[label(collection, "supporting location")]
    spans: Vec<miette::SourceSpan>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("formatted primary")]
struct FormattedPrimary {
    #[source_code]
    input: String,
    #[label(primary, "causal location")]
    cause: miette::SourceSpan,
    #[label("supporting location")]
    supporting: miette::SourceSpan,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum VariantLabels {
    #[error("ambiguous")]
    Ambiguous {
        #[source_code]
        input: String,
        #[label]
        first: miette::SourceSpan,
        #[label]
        second: miette::SourceSpan,
    },
    #[error("single")]
    Single {
        #[source_code]
        input: String,
        #[label]
        span: miette::SourceSpan,
    },
}

fn main() {}
