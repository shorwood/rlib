extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::section_analysis::SectionAnalyzer;
use crate::utils::test_module::CanonicalTestExt;

// -----------------------------------------------------------------------------
// Violation: Missing declaration family boundary diagnostic
// -----------------------------------------------------------------------------

/// Declaration family lacking the configured authored divider.
struct Violation {
    /// Unsectioned declaration-family span.
    span: Span,
    /// Analyzer-derived summary of the uncovered declarations.
    primary_message: String,
    /// Exact configured divider content proposed for the family.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "without explicit boundaries, readers must infer whether neighboring declarations form one responsibility or several",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MISSING_SECTION_DIVIDERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MissingSectionDividers: Responsibility boundary policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires authored dividers for declaration families.
struct MissingSectionDividers {
    /// Shared analyzer that parses and groups authored section dividers.
    analyzer: SectionAnalyzer,
}

impl MissingSectionDividers {
    /// Builds the pass from configured divider syntax.
    fn new() -> Self {
        Self {
            analyzer: SectionAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MISSING_SECTION_DIVIDERS,
    Warn,
    "requires section dividers for module-level declaration groups",
    MissingSectionDividers::new()
}

impl<'tcx> LateLintPass<'tcx> for MissingSectionDividers {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        if matches!(cx.tcx.hir_node(hir_id), Node::Item(item) if item.is_canonical_in_source_test_module(cx))
        {
            return;
        }
        for finding in self.analyzer.analyze(cx, module, hir_id).missing {
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
