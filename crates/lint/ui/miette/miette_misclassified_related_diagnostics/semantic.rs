#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("inner")]
struct Inner;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("inverted")]
struct InvertedRelated {
    #[related]
    source_errors: Vec<Inner>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("inverted")]
struct InvertedSource {
    #[diagnostic_source]
    related: Inner,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("inverted")]
struct ExpandedCausalVocabulary {
    #[related]
    root_cause: Vec<Inner>,
    #[related]
    r#underlying_error: Vec<Inner>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ExpandedSiblingVocabulary {
    #[error("inverted")]
    Inverted {
        #[diagnostic_source]
        related_diagnostics: Inner,
    },
    #[error("coherent")]
    Coherent {
        #[related]
        related_diagnostics: Vec<Inner>,
    },
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("coherent")]
struct Coherent {
    #[diagnostic_source]
    source: Inner,
    #[related]
    findings: Vec<Inner>,
}

fn main() {}
