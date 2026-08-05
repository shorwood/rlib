extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::lint::Level;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::function_structure::FunctionStructureAnalyzer;

// -----------------------------------------------------------------------------
// DeeplyNestedControlFlow
// -----------------------------------------------------------------------------

/// Late lint pass that reports the first control-flow construct crossing the depth limit.
struct DeeplyNestedControlFlow {
    /// Shared named-function analyzer configured for this lint family.
    analyzer: FunctionStructureAnalyzer,
}

impl DeeplyNestedControlFlow {
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
    /// Finds authored `if`, `match`, and loop constructs whose nesting exceeds the configured
    /// control-flow depth. `else if` chains remain one decision level, and closures are independent
    /// expressions rather than hidden extensions of their containing function.
    ///
    /// ### Why is this bad?
    ///
    /// Deep control flow multiplies the state a reader must retain and makes individual branches
    /// difficult to name or test. Guard clauses flatten avoidable depth; genuinely nested work
    /// should move behind a named helper.
    ///
    /// ```rust
    /// fn process(first: bool, second: bool, third: bool) {
    ///     if first {
    ///         while second {
    ///             if third {}
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// Extract the innermost operation or flatten guardable branches before adding another level.
    pub DEEPLY_NESTED_CONTROL_FLOW,
    Warn,
    "rejects control flow nesting beyond the configured depth",
    DeeplyNestedControlFlow::new()
}

impl<'tcx> LateLintPass<'tcx> for DeeplyNestedControlFlow {
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
            .deep_nesting
        {
            // Defer guardable cases to the more specific needless-nesting lint.
            let needless_level = cx
                .tcx
                .lint_level_at_node(
                    super::needlessly_nested_control_flow::NEEDLESSLY_NESTED_CONTROL_FLOW,
                    finding.hir_id,
                )
                .level;

            // Skip depth guidance when the enabled guard-clause lint is more specific.
            if finding.has_guard_clause_alternative && needless_level != Level::Allow {
                continue;
            }

            // Report excessive depth when no enabled guard-clause diagnostic supersedes it.
            cx.emit_span_lint(
                DEEPLY_NESTED_CONTROL_FLOW,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    diag.help(finding.help);
                }),
            );
        }
    }
}
