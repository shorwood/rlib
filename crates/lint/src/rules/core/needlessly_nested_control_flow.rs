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
// NeedlesslyNestedControlFlow
// -----------------------------------------------------------------------------

/// Late lint pass that replaces avoidable nesting with guard clauses.
struct NeedlesslyNestedControlFlow {
    /// Shared named-function analyzer configured for this lint family.
    analyzer: FunctionStructureAnalyzer,
}

impl NeedlesslyNestedControlFlow {
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
    /// Finds conditional structure that can be flattened through an early exit: useful work in an
    /// `else` beside a diverging branch, or a final `if`/`if let` that can use `return` or a
    /// loop-local `continue` as a guard.
    ///
    /// ### Why is this bad?
    ///
    /// Guard clauses make exceptional paths short and keep the main operation at the surrounding
    /// indentation level. Retaining an unnecessary branch visually presents the uncommon and common
    /// paths as equally important.
    ///
    /// ```rust
    /// fn process(valid: bool) {
    ///     if !valid {
    ///         return;
    ///     } else {
    ///         perform_work();
    ///     }
    /// }
    /// # fn perform_work() {}
    /// ```
    ///
    /// Remove the branch around the useful work:
    ///
    /// ```rust
    /// fn process(valid: bool) {
    ///     if !valid {
    ///         return;
    ///     }
    ///     perform_work();
    /// }
    /// # fn perform_work() {}
    /// ```
    pub NEEDLESSLY_NESTED_CONTROL_FLOW,
    Warn,
    "rejects conditional nesting that can be replaced by an early exit",
    NeedlesslyNestedControlFlow::new()
}

impl<'tcx> LateLintPass<'tcx> for NeedlesslyNestedControlFlow {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        def_id: LocalDefId,
    ) {
        if matches!(kind, FnKind::Closure) {
            return;
        }
        let unit = FunctionStructureAnalyzer::function_returns_unit(cx, def_id);
        for finding in self
            .analyzer
            .analyze_control_flow(cx, body, unit)
            .needless_nesting
        {
            cx.emit_span_lint(
                NEEDLESSLY_NESTED_CONTROL_FLOW,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    diag.help(finding.help);
                }),
            );
        }
    }
}
