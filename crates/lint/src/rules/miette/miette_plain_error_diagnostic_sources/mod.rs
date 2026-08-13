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
/// Performs the `check_fields` step of the lint analysis.
fn check_fields(cx: &LateContext<'_>, catalog: &DiagnosticCatalog, fields: &[DiagnosticField]) {
    for field in fields.iter().filter(|field| {
        field.roles.contains(DiagnosticFieldRole::Source)
            && !field.roles.contains(DiagnosticFieldRole::DiagnosticSource)
    }) {
        // Prepare the values used by this stage.
        let Some(target) = field.target else {
            continue;
        };
        if catalog.derived_type(target).is_none() {
            continue;
        }

        // Perform the next step of the analysis.
        Violation {
            span: field.span,
            field: field.name.clone(),
        }
        .emit(cx);
    }
}

#[derive(Default)]
/// Carries the `MiettePlainErrorDiagnosticSources` state used by this analysis.
struct MiettePlainErrorDiagnosticSources {
    /// Stores the `catalog` value used by this analysis.
    catalog: DiagnosticCatalog,
}
dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_PLAIN_ERROR_DIAGNOSTIC_SOURCES,
    Warn,
    "finds diagnostic sources forwarded only as standard errors",
    MiettePlainErrorDiagnosticSources::default()
}
impl LateLintPass<'_> for MiettePlainErrorDiagnosticSources {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            check_fields(cx, &self.catalog, &contract.fields);
            for member in &contract.members {
                check_fields(cx, &self.catalog, &member.fields);
            }
        }
    }
}
