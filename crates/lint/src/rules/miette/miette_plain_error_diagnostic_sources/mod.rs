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

// -----------------------------------------------------------------------------
// Violation: Diagnostic source exposed only as a plain error
// -----------------------------------------------------------------------------

/// Nested diagnostic whose structured metadata is not forwarded.
struct Violation {
    /// Source field declaration.
    span: Span,
    /// Source field name shown to the author.
    field: String,
}
impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "diagnostic source `{}` is exposed only as a plain error",
            self.field
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the inner type implements `miette::Diagnostic`, but only its standard error source chain is forwarded, losing codes, labels, help, and related diagnostics",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add `#[diagnostic_source]` while retaining `#[source]` for the standard error chain",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_PLAIN_ERROR_DIAGNOSTIC_SOURCES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "structured diagnostic metadata stops here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}
// -----------------------------------------------------------------------------
// MiettePlainErrorDiagnosticSources: Structured source forwarding
// -----------------------------------------------------------------------------

/// Collects diagnostic contracts before resolving nested source types.
#[derive(Default)]
struct MiettePlainErrorDiagnosticSources {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_PLAIN_ERROR_DIAGNOSTIC_SOURCES,
    Warn,
    "finds diagnostic sources forwarded only as standard errors",
    MiettePlainErrorDiagnosticSources::default()
}

impl MiettePlainErrorDiagnosticSources {
    /// Reports nested diagnostics marked only as standard error sources.
    fn check_fields(cx: &LateContext<'_>, catalog: &DiagnosticCatalog, fields: &[DiagnosticField]) {
        for field in fields.iter().filter(|field| {
            field.roles.contains(DiagnosticFieldRole::Source)
                && !field.roles.contains(DiagnosticFieldRole::DiagnosticSource)
        }) {
            let Some(target) = field.target else {
                continue;
            };
            if catalog.derived_type(target).is_none() {
                continue;
            }

            Violation {
                span: field.span,
                field: field.name.clone(),
            }
            .emit(cx);
        }
    }
}
impl LateLintPass<'_> for MiettePlainErrorDiagnosticSources {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            Self::check_fields(cx, &self.catalog, &contract.fields);
            for member in &contract.members {
                Self::check_fields(cx, &self.catalog, &member.fields);
            }
        }
    }
}
