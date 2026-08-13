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
    /// Stores the `labels` value used by this analysis.
    labels: Vec<String>,
}

impl Violation {
    /// Smallest label count for which a primary label is meaningful.
    const MINIMUM_COMPETING_LABELS: usize = 2;
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("multi-label diagnostic has no primary location")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "labels {} are presented without identifying the causal span",
            self.labels.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "mark the causal label `#[label(primary)]`; do not infer importance from field order",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_UNFOCUSED_DIAGNOSTIC_LABELS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this label set has no explicit focus");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}
/// Performs the `check_fields` step of the lint analysis.
fn check_fields(cx: &LateContext<'_>, span: Span, fields: &[DiagnosticField]) {
    // Prepare the values used by this stage.
    let labels = fields
        .iter()
        .filter(|field| field.roles.contains(DiagnosticFieldRole::Label))
        .collect::<Vec<_>>();

    // Reject inputs that do not satisfy this stage.
    if labels.len() < Violation::MINIMUM_COMPETING_LABELS
        || labels
            .iter()
            .any(|field| field.roles.contains(DiagnosticFieldRole::Primary))
    {
        return;
    }

    // Perform the next step of the analysis.
    Violation {
        span,
        labels: labels
            .into_iter()
            .map(|field| format!("`{}`", field.name))
            .collect(),
    }
    .emit(cx);
}

#[derive(Default)]
/// Carries the `MietteUnfocusedDiagnosticLabels` state used by this analysis.
struct MietteUnfocusedDiagnosticLabels {
    /// Stores the `catalog` value used by this analysis.
    catalog: DiagnosticCatalog,
}
dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_UNFOCUSED_DIAGNOSTIC_LABELS,
    Warn,
    "finds multi-label Miette diagnostics without a primary label",
    MietteUnfocusedDiagnosticLabels::default()
}
impl LateLintPass<'_> for MietteUnfocusedDiagnosticLabels {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            check_fields(cx, contract.span, &contract.fields);
            for member in &contract.members {
                check_fields(cx, member.span, &member.fields);
            }
        }
    }
}
