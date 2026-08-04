extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::section_dividers::DividerAnalyzer;

// -----------------------------------------------------------------------------
// MissingSectionDividers
// -----------------------------------------------------------------------------

struct MissingSectionDividers {
    analyzer: DividerAnalyzer,
}

impl MissingSectionDividers {
    fn new() -> Self {
        Self {
            analyzer: DividerAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Requires module-level type families to be covered by a configured section divider.
    /// Structs, enums, unions, traits, type aliases, and direct impls participate; other items do
    /// not affect section membership.
    ///
    /// ### Why is this bad?
    ///
    /// A divider makes the intended naming family explicit. Without one, agents cannot tell whether
    /// neighboring declarations are deliberately related or merely accumulated in the same file.
    ///
    /// For example, these declarations have no stated family:
    ///
    /// ```rust
    /// struct Request;
    /// impl Request {}
    /// ```
    ///
    /// A divider establishes the naming contract:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request: Request model and behavior
    /// // -----------------------------------------------------------------------------
    ///
    /// struct Request;
    /// impl Request {}
    /// ```
    pub MISSING_SECTION_DIVIDERS,
    Warn,
    "requires section dividers for module-level type families",
    MissingSectionDividers::new()
}

impl<'tcx> LateLintPass<'tcx> for MissingSectionDividers {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, _: HirId) {
        for finding in self.analyzer.analyze(cx, module).missing {
            cx.emit_span_lint(
                MISSING_SECTION_DIVIDERS,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    diag.help(finding.help);
                }),
            );
        }
    }
}
