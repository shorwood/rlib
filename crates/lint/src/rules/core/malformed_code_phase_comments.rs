extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::function_structure::FunctionStructureAnalyzer;

// -----------------------------------------------------------------------------
// MalformedCodePhaseComments
// -----------------------------------------------------------------------------

/// Late lint pass that validates authored code-phase comment syntax and placement.
struct MalformedCodePhaseComments {
    /// Shared named-function analyzer configured for this lint family.
    analyzer: FunctionStructureAnalyzer,
}

impl MalformedCodePhaseComments {
    /// Builds the pass from validated function-structure configuration.
    fn new() -> Self {
        Self {
            analyzer: FunctionStructureAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Validates explanatory comment blocks inside named functions and methods. The first line must
    /// use the configured prefix and sentence-style prose, continuation lines may wrap naturally,
    /// and the block must immediately precede the code it explains. Subsequent blocks require a
    /// separating blank line.
    ///
    /// ### Why is this bad?
    ///
    /// Decorative, empty, or inconsistently placed comments do not explain the code they divide.
    /// Natural prose makes a boundary useful to a reader instead of turning it into a branded or
    /// mechanical line-count escape hatch.
    ///
    /// This header has no purpose and is separated from its code:
    ///
    /// ```rust
    /// fn prepare() {
    ///     //
    ///
    ///     let input = String::new();
    /// }
    /// ```
    ///
    /// Put concise sentence-style prose directly before the phase:
    ///
    /// ```rust
    /// fn prepare() {
    ///     // Read and normalize the input state.
    ///     let input = String::new();
    /// }
    /// ```
    pub MALFORMED_CODE_PHASE_COMMENTS,
    Warn,
    "rejects malformed or misplaced code phase comments",
    MalformedCodePhaseComments::new()
}

impl<'tcx> LateLintPass<'tcx> for MalformedCodePhaseComments {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        _: LocalDefId,
    ) {
        if matches!(kind, FnKind::Closure) || span.from_expansion() {
            return;
        }
        for finding in self.analyzer.analyze_layout(cx, body).malformed {
            cx.emit_span_lint(
                MALFORMED_CODE_PHASE_COMMENTS,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    if let Some(replacement) = finding.replacement {
                        diag.span_suggestion(
                            finding.span,
                            "render this phase comment canonically",
                            replacement,
                            Applicability::MachineApplicable,
                        );
                    } else {
                        diag.help(finding.help);
                    }
                }),
            );
        }
    }
}
