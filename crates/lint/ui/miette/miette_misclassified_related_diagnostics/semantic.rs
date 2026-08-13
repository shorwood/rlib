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
#[error("coherent")]
struct Coherent {
    #[diagnostic_source]
    source: Inner,
    #[related]
    findings: Vec<Inner>,
}

fn main() {}
