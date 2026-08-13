extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::contracts::DiagnosticCatalog;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    span: Span,
    diagnostic: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "diagnostic `{}` has no stable code",
            self.diagnostic
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "sibling variants establish codes as this diagnostic family's machine identity, but this variant cannot be filtered or tracked equivalently",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("add a unique namespaced `#[diagnostic(code(...))]` to this variant")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_MISSING_DIAGNOSTIC_CODES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this variant breaks the family's code policy");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct MietteMissingDiagnosticCodes {
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_MISSING_DIAGNOSTIC_CODES,
    Warn,
    "finds code-less variants in coded Miette diagnostic families",
    MietteMissingDiagnosticCodes::default()
}

impl LateLintPass<'_> for MietteMissingDiagnosticCodes {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            let family_has_codes = contract.metadata.code.is_some()
                || contract
                    .members
                    .iter()
                    .any(|member| member.metadata.code.is_some());
            if !family_has_codes {
                continue;
            }
            for member in &contract.members {
                if !member.metadata.transparent
                    && member.metadata.code.is_none()
                    && contract.metadata.code.is_none()
                {
                    Violation {
                        span: member.span,
                        diagnostic: format!("{}::{}", contract.name, member.name),
                    }
                    .emit(cx);
                }
            }
        }
    }
}
