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

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("invalid credential")]
struct QualifiedNames {
    #[source_code]
    customer_access_token_payload: String,
    #[label]
    span: miette::SourceSpan,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum VariantSources {
    #[error("password rejected")]
    Password {
        #[source_code]
        password_excerpt: String,
        #[label]
        span: miette::SourceSpan,
    },
    #[error("private content rejected")]
    Private {
        #[source_code]
        r#private_content: String,
        #[label]
        span: miette::SourceSpan,
    },
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("redacted")]
struct ExplicitlyRedacted {
    #[source_code]
    redacted_api_key: String,
    #[label]
    span: miette::SourceSpan,
}

fn main() {}
