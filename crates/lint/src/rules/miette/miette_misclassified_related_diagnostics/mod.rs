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

enum Kind {
    CauseAsRelated,
    SiblingAsCause,
}
struct Violation {
    span: Span,
    field: String,
    kind: Kind,
}
impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(match self.kind {
            Kind::CauseAsRelated => format!(
                "causal field `{}` is classified as a related diagnostic",
                self.field
            ),
            Kind::SiblingAsCause => format!(
                "sibling field `{}` is classified as the diagnostic source",
                self.field
            ),
        })
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the declared Miette relationship contradicts the field's explicit domain role and distorts the rendered diagnostic tree",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(match self.kind {
            Kind::CauseAsRelated => {
                "model the primary failure with `#[diagnostic_source]` and reserve `#[related]` for sibling findings"
            }
            Kind::SiblingAsCause => {
                "model independent findings with `#[related]` and select one actual causal diagnostic source"
            }
        })
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_MISCLASSIFIED_RELATED_DIAGNOSTICS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this relationship conflicts with the field role");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}
#[derive(Default)]
struct MietteMisclassifiedRelatedDiagnostics {
    catalog: DiagnosticCatalog,
}
dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_MISCLASSIFIED_RELATED_DIAGNOSTICS,
    Warn,
    "finds inverted Miette causal and related roles",
    MietteMisclassifiedRelatedDiagnostics::default()
}
impl LateLintPass<'_> for MietteMisclassifiedRelatedDiagnostics {
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
fn check_fields(cx: &LateContext<'_>, fields: &[DiagnosticField]) {
    for field in fields {
        let name = field.name.to_ascii_lowercase();
        let kind = if field.roles.contains(DiagnosticFieldRole::Related) && causal_name(&name) {
            Some(Kind::CauseAsRelated)
        } else if field.roles.contains(DiagnosticFieldRole::DiagnosticSource) && sibling_name(&name)
        {
            Some(Kind::SiblingAsCause)
        } else {
            None
        };
        if let Some(kind) = kind {
            Violation {
                span: field.span,
                field: field.name.clone(),
                kind,
            }
            .emit(cx);
        }
    }
}
fn causal_name(name: &str) -> bool {
    matches!(
        name,
        "source" | "cause" | "error" | "root_error" | "source_error" | "source_errors"
    )
}
fn sibling_name(name: &str) -> bool {
    matches!(
        name,
        "related" | "findings" | "warnings" | "suppressed" | "alternatives"
    )
}
