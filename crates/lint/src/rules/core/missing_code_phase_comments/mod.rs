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

/// Oversized direct statement phase without an authored explanation.
struct Violation {
    /// Continuous phase span used as the primary diagnostic location.
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
            "the uninterrupted statements conceal the purpose and boundary of this stage of the function",
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
