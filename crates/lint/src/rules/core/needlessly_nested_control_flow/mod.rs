extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::function_structure::FunctionStructureAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Avoidable control flow nesting diagnostic
// -----------------------------------------------------------------------------

/// Conditional structure that can expose its main path through an early exit.
struct Violation {
    /// Avoidably nested construct span.
    span: Span,
    /// Analyzer-derived description of the concrete nesting shape.
    primary_message: String,
    /// Analyzer-derived guard-clause rewrite guidance.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the exceptional path keeps the main operation indented and makes both paths appear equally important",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            NEEDLESSLY_NESTED_CONTROL_FLOW,
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
// NeedlesslyNestedControlFlow: Guard-clause policy
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
    #[doc = include_str!("README.md")]
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
        for finding in self
            .analyzer
            .analyze_control_flow(cx, body, def_id)
            .needless_nesting
        {
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
