#![allow(dead_code, rlib::miette_labels_without_source_code, unknown_lints)]

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

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("nested without focus")]
struct NestedWithoutFocus;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("nested with focus")]
struct NestedWithFocus {
    #[label]
    span: miette::SourceSpan,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("related but unfocused")]
struct UnfocusedRelated {
    #[source_code]
    input: String,
    #[related]
    findings: Vec<NestedWithoutFocus>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("related and focused")]
struct FocusedRelated {
    #[source_code]
    input: String,
    #[related]
    findings: Vec<NestedWithFocus>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("source but unfocused")]
struct UnfocusedDiagnosticSource {
    #[source_code]
    input: String,
    #[diagnostic_source]
    nested: NestedWithoutFocus,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum VariantFocus {
    #[error("unfocused")]
    Unfocused {
        #[source_code]
        input: String,
    },
    #[error("focused")]
    Focused {
        #[source_code]
        input: String,
        #[label]
        span: miette::SourceSpan,
    },
}

fn main() {}
