extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::contracts::DiagnosticCatalog;
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    span: Span,
    help: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!("diagnostic help `{}` is not actionable", self.help))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "generic advice does not tell the reader which concrete action can resolve this diagnostic",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace the generic phrase with a specific action, expected value, path, or alternative",
        )
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_GENERIC_DIAGNOSTIC_HELP,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this help text lacks a concrete remediation");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct MietteGenericDiagnosticHelp {
    catalog: DiagnosticCatalog,
    generic_phrases: Vec<String>,
}

impl MietteGenericDiagnosticHelp {
    fn new() -> Self {
        Self {
            catalog: DiagnosticCatalog::default(),
            generic_phrases: LibraryConfig::load().miette_help.generic_phrases,
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_GENERIC_DIAGNOSTIC_HELP,
    Warn,
    "finds non-actionable static Miette help text",
    MietteGenericDiagnosticHelp::new()
}

impl LateLintPass<'_> for MietteGenericDiagnosticHelp {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }
    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            self.check_help(cx, contract.span, contract.metadata.help.as_deref());
            for member in &contract.members {
                self.check_help(cx, member.span, member.metadata.help.as_deref());
            }
        }
    }
}

impl MietteGenericDiagnosticHelp {
    fn check_help(&self, cx: &LateContext<'_>, span: Span, help: Option<&str>) {
        let Some(help) = help else {
            return;
        };
        let normalized = help
            .trim()
            .trim_end_matches(['.', '!'])
            .to_ascii_lowercase();
        if self
            .generic_phrases
            .iter()
            .any(|phrase| normalized == phrase.trim().to_ascii_lowercase())
        {
            Violation {
                span,
                help: help.to_owned(),
            }
            .emit(cx);
        }
    }
}
