extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::contracts::{DiagnosticCatalog, DiagnosticField};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    span: Span,
    labels: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("diagnostic labels have no source code")
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "label fields {} cannot render excerpts without a local or forwarded source",
            self.labels.join(", ")
        ))
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add a `#[source_code]` field, forward a diagnostic source, or remove the unusable labels",
        )
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_LABELS_WITHOUT_SOURCE_CODE,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "these labels cannot resolve against source text");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct MietteLabelsWithoutSourceCode {
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_LABELS_WITHOUT_SOURCE_CODE,
    Warn,
    "finds Miette labels without an available source",
    MietteLabelsWithoutSourceCode::default()
}

impl LateLintPass<'_> for MietteLabelsWithoutSourceCode {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }
    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            check_fields(
                cx,
                contract.span,
                &contract.fields,
                contract.metadata.transparent,
            );
            for member in &contract.members {
                check_fields(
                    cx,
                    member.span,
                    &member.fields,
                    member.metadata.transparent || contract.metadata.transparent,
                );
            }
        }
    }
}

fn check_fields(cx: &LateContext<'_>, span: Span, fields: &[DiagnosticField], transparent: bool) {
    let labels = fields
        .iter()
        .filter(|field| field.roles.label)
        .collect::<Vec<_>>();
    if labels.is_empty()
        || transparent
        || fields
            .iter()
            .any(|field| field.roles.source_code || field.roles.diagnostic_source)
    {
        return;
    }
    Violation {
        span,
        labels: labels
            .into_iter()
            .map(|field| format!("`{}`", field.name))
            .collect(),
    }
    .emit(cx);
}
