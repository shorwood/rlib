extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::section_dividers::DividerAnalyzer;

// -----------------------------------------------------------------------------
// MismatchedSectionDividerPrefixes
// -----------------------------------------------------------------------------

struct MismatchedSectionDividerPrefixes {
    analyzer: DividerAnalyzer,
}

impl MismatchedSectionDividerPrefixes {
    fn new() -> Self {
        Self {
            analyzer: DividerAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Requires a divider prefix to equal the longest PascalCase word prefix shared by every
    /// participating declaration in its section.
    ///
    /// ### Why is this bad?
    ///
    /// A mismatched or vague prefix conceals inconsistent names. The preferred remedy is to rename
    /// related outliers into a followable family, not to create a section for every declaration.
    ///
    /// For example, Response does not belong to the declared Request family:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// struct Response;
    /// ```
    ///
    /// Rename a related declaration so the family is visible:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// struct RequestResponse;
    /// ```
    pub MISMATCHED_SECTION_DIVIDER_PREFIXES,
    Warn,
    "requires divider prefixes to match their declaration families",
    MismatchedSectionDividerPrefixes::new()
}

impl<'tcx> LateLintPass<'tcx> for MismatchedSectionDividerPrefixes {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, _: HirId) {
        for finding in self.analyzer.analyze(cx, module).mismatches {
            cx.emit_span_lint(
                MISMATCHED_SECTION_DIVIDER_PREFIXES,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    diag.help(finding.help);
                }),
            );
        }
    }
}
