extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::lint::Level;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::function_structure::FunctionStructureAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Excessive control flow depth diagnostic
// -----------------------------------------------------------------------------

/// Control-flow construct whose resolved nesting exceeds the configured limit.
struct Violation {
    /// Construct span used as the primary diagnostic location.
    span: Span,
    /// Analyzer-derived message containing the measured depth.
    primary_message: String,
    /// Analyzer-derived remediation tailored to the construct.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "each additional nesting level increases the branch state a reader must retain before reaching this operation",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DEEPLY_NESTED_CONTROL_FLOW,
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
        for finding in self
            .analyzer
            .analyze_control_flow(cx, body, def_id)
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
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
