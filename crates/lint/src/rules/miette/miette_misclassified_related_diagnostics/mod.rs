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
// Violation: Diagnostic relationship contradicting its domain role
// -----------------------------------------------------------------------------

/// Direction in which a field's declared relationship is inverted.
enum ViolationKind {
    /// A causal field is presented as an independent sibling.
    CauseAsRelated,
    /// A sibling collection is presented as the primary cause.
    SiblingAsCause,
}

/// Field whose Miette relationship conflicts with its explicit name.
struct Violation {
    /// Misclassified field declaration.
    span: Span,
    /// Field name shown to the author.
    field: String,
    /// Detected relationship inversion.
    kind: ViolationKind,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(match self.kind {
            ViolationKind::CauseAsRelated => format!(
                "causal field `{}` is classified as a related diagnostic",
                self.field
            ),
            ViolationKind::SiblingAsCause => format!(
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
            ViolationKind::CauseAsRelated => {
                "model the primary failure with `#[diagnostic_source]` and reserve `#[related]` for sibling findings"
            }
            ViolationKind::SiblingAsCause => {
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

// -----------------------------------------------------------------------------
// MietteMisclassifiedRelatedDiagnostics: Causal-tree relationship policy
// -----------------------------------------------------------------------------

/// Collects diagnostic field roles before checking relationship intent.
#[derive(Default)]
struct MietteMisclassifiedRelatedDiagnostics {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_MISCLASSIFIED_RELATED_DIAGNOSTICS,
    Warn,
    "finds inverted Miette causal and related roles",
    MietteMisclassifiedRelatedDiagnostics::default()
}

impl MietteMisclassifiedRelatedDiagnostics {
    /// Recognizes field names that explicitly claim a causal role.
    fn is_causal_name(name: &str) -> bool {
        matches!(
            name,
            "source"
                | "cause"
                | "causes"
                | "error"
                | "errors"
                | "root_cause"
                | "root_causes"
                | "root_error"
                | "root_errors"
                | "source_error"
                | "source_errors"
                | "underlying_error"
                | "underlying_errors"
        )
    }

    /// Recognizes field names that explicitly claim a sibling role.
    fn is_sibling_name(name: &str) -> bool {
        matches!(
            name,
            "related"
                | "related_diagnostic"
                | "related_diagnostics"
                | "finding"
                | "findings"
                | "warning"
                | "warnings"
                | "suppressed"
                | "suppressed_error"
                | "suppressed_errors"
                | "alternative"
                | "alternatives"
                | "issue"
                | "issues"
                | "notice"
                | "notices"
        )
    }

    /// Reports field names and Miette roles that point in opposite directions.
    fn check_fields(cx: &LateContext<'_>, fields: &[DiagnosticField]) {
        for field in fields {
            let name = field
                .name
                .strip_prefix("r#")
                .unwrap_or(&field.name)
                .to_ascii_lowercase();
            let kind = if field.roles.contains(DiagnosticFieldRole::Related)
                && Self::is_causal_name(&name)
            {
                Some(ViolationKind::CauseAsRelated)
            } else if field.roles.contains(DiagnosticFieldRole::DiagnosticSource)
                && Self::is_sibling_name(&name)
            {
                Some(ViolationKind::SiblingAsCause)
            } else {
                None
            };

            let Some(kind) = kind else {
                continue;
            };

            Violation {
                span: field.span,
                field: field.name.clone(),
                kind,
            }
            .emit(cx);
        }
    }
}

impl LateLintPass<'_> for MietteMisclassifiedRelatedDiagnostics {
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
