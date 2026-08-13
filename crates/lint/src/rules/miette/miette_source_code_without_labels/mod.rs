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

struct Violation {
    span: Span,
    sources: Vec<String>,
}
impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("diagnostic retains source code without a focus")
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "source fields {} have no label, related diagnostic, or diagnostic source that can select a relevant location",
            self.sources.join(", ")
        ))
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add focused label metadata or remove source-code storage from this diagnostic",
        )
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_SOURCE_CODE_WITHOUT_LABELS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this source has no diagnostic focus");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct MietteSourceCodeWithoutLabels {
    catalog: DiagnosticCatalog,
}
dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_SOURCE_CODE_WITHOUT_LABELS,
    Warn,
    "finds unfocused Miette source-code storage",
    MietteSourceCodeWithoutLabels::default()
}
impl LateLintPass<'_> for MietteSourceCodeWithoutLabels {
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
    let sources = fields
        .iter()
        .filter(|field| field.roles.contains(DiagnosticFieldRole::SourceCode))
        .collect::<Vec<_>>();
    if sources.is_empty()
        || transparent
        || fields.iter().any(|field| {
            field.roles.contains(DiagnosticFieldRole::Label)
                || field.roles.contains(DiagnosticFieldRole::Related)
                || field.roles.contains(DiagnosticFieldRole::DiagnosticSource)
        })
    {
        return;
    }
    Violation {
        span,
        sources: sources
            .into_iter()
            .map(|field| format!("`{}`", field.name))
            .collect(),
    }
    .emit(cx);
}
