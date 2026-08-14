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

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("nested")]
struct Nested;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("related findings still need source text for their own label")]
struct RelatedIsNotSource {
    #[label("first")]
    first: miette::SourceSpan,
    #[label(primary)]
    second: miette::SourceSpan,
    #[related]
    findings: Vec<Nested>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum VariantContracts {
    #[error("missing source")]
    Missing {
        #[label]
        span: miette::SourceSpan,
    },
    #[error("complete")]
    Complete {
        #[source_code]
        input: String,
        #[label]
        span: miette::SourceSpan,
    },
    #[error("forwarded")]
    Forwarded {
        #[diagnostic_source]
        nested: Nested,
        #[label]
        span: miette::SourceSpan,
    },
}

fn main() {}
