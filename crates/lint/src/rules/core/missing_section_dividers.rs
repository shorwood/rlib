extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::source_organization::SectionAnalyzer;

// -----------------------------------------------------------------------------
// MissingSectionDividers
// -----------------------------------------------------------------------------

struct MissingSectionDividers {
    analyzer: SectionAnalyzer,
}

impl MissingSectionDividers {
    fn new() -> Self {
        Self {
            analyzer: SectionAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Requires module-level declaration groups to be covered by a configured section divider.
    /// Nominal types and their direct impls always participate. Free functions, constants, and
    /// statics also participate when they form a group or occur inside an authored section; an
    /// isolated value declaration does not require a divider by itself.
    ///
    /// ### Why is this bad?
    ///
    /// A divider makes the intended naming family explicit. Without one, agents cannot tell
    /// whether neighboring declarations are deliberately related or merely accumulated in the
    /// same file. Ignoring free helpers also lets a nominally valid section conceal inconsistent
    /// vocabulary.
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
    "requires section dividers for module-level declaration groups",
    MissingSectionDividers::new()
}

impl<'tcx> LateLintPass<'tcx> for MissingSectionDividers {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id).missing {
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
