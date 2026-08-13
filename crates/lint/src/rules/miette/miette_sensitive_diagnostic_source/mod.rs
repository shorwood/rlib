extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;
use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::contracts::{DiagnosticCatalog, DiagnosticField, DiagnosticFieldRole};
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

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `field` value used by this analysis.
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
/// Performs the `sensitive_name` step of the lint analysis.
fn sensitive_name(name: &str) -> bool {
    // Prepare the values used by this stage.
    let name = name.to_ascii_lowercase();

    // Perform the next step of the analysis.
    SENSITIVE_TERMS
        .iter()
        .any(|term| name == *term || name.ends_with(&format!("_{term}")))
}
/// Performs the `check_fields` step of the lint analysis.
fn check_fields(cx: &LateContext<'_>, fields: &[DiagnosticField]) {
    for field in fields.iter().filter(|field| {
        field.roles.contains(DiagnosticFieldRole::SourceCode) && sensitive_name(&field.name)
    }) {
        Violation {
            span: field.span,
            field: field.name.clone(),
        }
        .emit(cx);
    }
}

#[derive(Default)]
/// Carries the `MietteSensitiveDiagnosticSource` state used by this analysis.
struct MietteSensitiveDiagnosticSource {
    /// Stores the `catalog` value used by this analysis.
    catalog: DiagnosticCatalog,
}
dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_SENSITIVE_DIAGNOSTIC_SOURCE,
    Warn,
    "finds sensitive fields exposed as Miette source code",
    MietteSensitiveDiagnosticSource::default()
}
impl LateLintPass<'_> for MietteSensitiveDiagnosticSource {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            check_fields(cx, &contract.fields);
            for member in &contract.members {
                check_fields(cx, &member.fields);
            }
        }
    }
}
