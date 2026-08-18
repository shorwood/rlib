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
// Violation: Missing code phase explanation diagnostic
// -----------------------------------------------------------------------------

/// Authored function phase whose purpose or scope remains unclear.
struct Violation {
    /// Phase span used as the primary diagnostic location.
    span: Span,
    /// Analyzer-derived message containing the measured phase size.
    primary_message: String,
    /// Analyzer-derived comment guidance for this phase.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "visible phase boundaries should name a real transition, and each named phase should remain small enough to follow",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MISSING_CODE_PHASE_COMMENTS,
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
// MissingCodePhaseComments: Authored workflow boundary policy
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

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
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
        // Closures and generated bodies do not expose authored function phases here.
        if matches!(kind, FnKind::Closure) || span.from_expansion() || span.is_build_generated(cx) {
            return;
        }
        for finding in self.analyzer.analyze_layout(cx, body).missing {
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
