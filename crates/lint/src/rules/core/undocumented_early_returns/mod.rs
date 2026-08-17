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
use crate::utils::source_provenance::SpanProvenanceExt;

// -----------------------------------------------------------------------------
// Violation: Unexplained early return diagnostic
// -----------------------------------------------------------------------------

/// Authored early return whose guard boundary has no code-phase explanation.
struct Violation {
    /// Explicit return expression receiving the primary diagnostic.
    return_span: Span,
    /// Preferred guard-level location for the missing explanation.
    boundary_span: Span,
    /// Configuration-aware comment guidance.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this early return has no explanatory comment")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "an early exit introduces policy that the guard condition alone may not communicate",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            UNDOCUMENTED_EARLY_RETURNS,
            self.return_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.return_span, "this exits before the remaining work");
                if self.boundary_span != self.return_span {
                    diag.span_label(self.boundary_span, "explain this guard boundary");
                }
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// UndocumentedEarlyReturns: Guard explanation policy
// -----------------------------------------------------------------------------

/// Late lint pass requiring explanations on authored explicit early returns.
struct UndocumentedEarlyReturns {
    /// Shared named-function and code-phase analysis.
    analyzer: FunctionStructureAnalyzer,
}

impl UndocumentedEarlyReturns {
    /// Builds the pass from validated function-structure configuration.
    fn new() -> Self {
        Self {
            analyzer: FunctionStructureAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub UNDOCUMENTED_EARLY_RETURNS,
    Warn,
    "requires explicit early returns to explain their guard policy",
    UndocumentedEarlyReturns::new()
}

impl<'tcx> LateLintPass<'tcx> for UndocumentedEarlyReturns {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        _: LocalDefId,
    ) {
        // Generated bodies contain no authored guard comments to enforce.
        if span.from_expansion() || span.is_build_generated(cx) {
            return;
        }
        for finding in self.analyzer.analyze_early_returns(cx, body) {
            Violation {
                return_span: finding.return_span,
                boundary_span: finding.boundary_span,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
