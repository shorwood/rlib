extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::source_organization::SectionAnalyzer;

// -----------------------------------------------------------------------------
// DuplicateSectionDividerPrefixes
// -----------------------------------------------------------------------------

struct DuplicateSectionDividerPrefixes {
    analyzer: SectionAnalyzer,
}

impl DuplicateSectionDividerPrefixes {
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
            cx.emit_span_lint(
                DUPLICATE_SECTION_DIVIDER_PREFIXES,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    diag.help(finding.help);
                }),
            );
        }
    }
}
