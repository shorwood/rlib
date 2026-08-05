extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::function_structure::FunctionStructureAnalyzer;

// -----------------------------------------------------------------------------
// MissingCodePhaseComments
// -----------------------------------------------------------------------------

/// Late lint pass that requires explanations for oversized direct code phases.
struct MissingCodePhaseComments {
    /// Shared named-function analyzer configured for this lint family.
    analyzer: FunctionStructureAnalyzer,
}

impl MissingCodePhaseComments {
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
    /// Finds named functions and methods whose direct block surface exceeds the configured limit
    /// without being divided into short, explanatory phases. Nested authored blocks are measured
    /// independently so their implementation does not inflate the containing phase. The physical
    /// line limit and comment prefix are configurable through the shared `function_structure`
    /// table.
    ///
    /// ### Why is this bad?
    ///
    /// A long uninterrupted sequence forces readers to reconstruct where preparation ends and the
    /// next operation begins. Natural prose comments provide navigation when the work remains
    /// inherently sequential. Treating control flow as self-explanatory hides mixed workflows,
    /// while counting nested bodies against their parent reports the same complexity twice.
    ///
    /// For a three-line limit, this run has no named phases:
    ///
    /// ```rust
    /// fn prepare() {
    ///     let input = String::new();
    ///     let trimmed = input.trim();
    ///     let length = trimmed.len();
    ///     let empty = trimmed.is_empty();
    /// }
    /// ```
    ///
    /// Explain every phase, including the first, and keep each below the configured limit:
    ///
    /// ```rust
    /// fn prepare() {
    ///     // Read and normalize the input before deriving its properties.
    ///     let input = String::new();
    ///     let trimmed = input.trim();
    ///
    ///     // Derive the properties consumed by the caller.
    ///     let length = trimmed.len();
    ///     let empty = trimmed.is_empty();
    /// }
    /// ```
    pub MISSING_CODE_PHASE_COMMENTS,
    Warn,
    "requires long direct code phases to be divided by explanatory comments",
    MissingCodePhaseComments::new()
}

impl<'tcx> LateLintPass<'tcx> for MissingCodePhaseComments {
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
        for finding in self.analyzer.analyze_layout(cx, body).missing {
            cx.emit_span_lint(
                MISSING_CODE_PHASE_COMMENTS,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    diag.help(finding.help);
                }),
            );
        }
    }
}
