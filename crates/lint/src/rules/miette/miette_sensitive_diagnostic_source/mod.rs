extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;
use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::utils::contracts::{DiagnosticCatalog, DiagnosticField, DiagnosticFieldRole};
use crate::utils::diagnostic::LateViolation;

/// Field names that commonly carry confidential content.
const SENSITIVE_TERMS: &[&str] = &[
    "password",
    "passphrase",
    "access_token",
    "refresh_token",
    "auth_token",
    "api_token",
    "api_key",
    "private_key",
    "client_secret",
    "shared_secret",
    "request_body",
    "response_body",
    "private_content",
];

// -----------------------------------------------------------------------------
// Violation: Sensitive content exposed as source code
// -----------------------------------------------------------------------------

/// Source-code field whose name indicates confidential content.
struct Violation {
    /// Sensitive field declaration.
    span: Span,
    /// Field name shown to the author.
    field: String,
}
impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "sensitive field `{}` is exposed as diagnostic source code",
            self.field
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Miette reporters may print labeled excerpts and surrounding text from this field to terminals, logs, or serialized reports",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "store a redacted source representation or keep sensitive content outside the diagnostic",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_SENSITIVE_DIAGNOSTIC_SOURCE,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this source can disclose sensitive content");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}
// -----------------------------------------------------------------------------
// MietteSensitiveDiagnosticSource: Confidential source-rendering policy
// -----------------------------------------------------------------------------

/// Collects diagnostic field roles before checking sensitive source exposure.
#[derive(Default)]
struct MietteSensitiveDiagnosticSource {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_SENSITIVE_DIAGNOSTIC_SOURCE,
    Warn,
    "finds sensitive fields exposed as Miette source code",
    MietteSensitiveDiagnosticSource::default()
}

impl MietteSensitiveDiagnosticSource {
    /// Matches exact and qualified sensitive field names.
    fn sensitive_name(name: &str) -> bool {
        let name = name.strip_prefix("r#").unwrap_or(name).to_ascii_lowercase();

        // Explicit redaction vocabulary marks the source as intentionally sanitized.
        if name
            .split('_')
            .any(|component| matches!(component, "masked" | "redacted" | "sanitized" | "scrubbed"))
        {
            return false;
        }

        SENSITIVE_TERMS.iter().any(|term| {
            name == *term
                || name.starts_with(&format!("{term}_"))
                || name.ends_with(&format!("_{term}"))
                || name.contains(&format!("_{term}_"))
        })
    }

    /// Reports sensitive fields exposed through Miette's source rendering.
    fn check_fields(cx: &LateContext<'_>, fields: &[DiagnosticField]) {
        for field in fields.iter().filter(|field| {
            field.roles.contains(DiagnosticFieldRole::SourceCode)
                && Self::sensitive_name(&field.name)
        }) {
            Violation {
                span: field.span,
                field: field.name.clone(),
            }
            .emit(cx);
        }
    }
}
impl LateLintPass<'_> for MietteSensitiveDiagnosticSource {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            Self::check_fields(cx, &contract.fields);
            for member in &contract.members {
                Self::check_fields(cx, &member.fields);
            }
        }
    }
}
