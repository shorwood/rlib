extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::section_analysis::SectionAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Duplicate section family diagnostic
// -----------------------------------------------------------------------------

/// Repeated section prefix with analyzer-derived consolidation guidance.
struct Violation {
    /// Repeated divider span used as the primary diagnostic site.
    span: Span,
    /// Analyzer-derived duplicate-prefix summary.
    primary_message: String,
    /// Consolidation guidance naming the earlier owning section.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "reusing one prefix fragments a naming family and leaves new declarations without an unambiguous owning section",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DUPLICATE_SECTION_DIVIDER_PREFIXES,
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
// DuplicateSectionDividerPrefixes
// -----------------------------------------------------------------------------

/// Late lint pass that rejects reused section family prefixes.
struct DuplicateSectionDividerPrefixes {
    /// Shared analyzer that parses and groups authored section dividers.
    analyzer: SectionAnalyzer,
}

impl DuplicateSectionDividerPrefixes {
    /// Builds the pass from configured divider syntax.
    fn new() -> Self {
        Self {
            analyzer: SectionAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Rejects repeated section-divider prefixes within the same module. Nested modules have
    /// independent prefix namespaces.
    ///
    /// ### Why is this bad?
    ///
    /// Repeating a prefix fragments one naming family and makes it unclear which section owns new
    /// declarations. Closely related declarations should remain together.
    ///
    /// For example, this module splits the Request family:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    ///
    /// // -----------------------------------------------------------------------------
    /// // Request
    /// // -----------------------------------------------------------------------------
    /// struct RequestBuilder;
    /// ```
    ///
    /// Keep the family under one divider:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// struct RequestBuilder;
    /// ```
    pub DUPLICATE_SECTION_DIVIDER_PREFIXES,
    Warn,
    "rejects duplicate section divider prefixes within a module",
    DuplicateSectionDividerPrefixes::new()
}

impl<'tcx> LateLintPass<'tcx> for DuplicateSectionDividerPrefixes {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id).duplicates {
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
