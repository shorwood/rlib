extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::function_structure::FunctionStructureAnalyzer;
use crate::utils::source_provenance::SpanProvenanceExt;

// -----------------------------------------------------------------------------
// Violation: Malformed code phase explanation diagnostic
// -----------------------------------------------------------------------------

/// Authored phase comment whose syntax or placement violates the configured form.
struct Violation {
    /// Comment span used for the diagnostic and optional replacement.
    span: Span,
    /// Analyzer-derived description of the malformed comment.
    primary_message: String,
    /// Analyzer-derived manual remediation when no safe replacement exists.
    remediation_message: String,
    /// Canonical replacement when the existing comment can be preserved safely.
    replacement: Option<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "inconsistent phase markers make workflow boundaries harder for readers and tools to recognize reliably",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        if self.replacement.is_some() {
            Cow::Borrowed("render this phase comment canonically")
        } else {
            Cow::Borrowed(&self.remediation_message)
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();
        cx.emit_span_lint(
            MALFORMED_CODE_PHASE_COMMENTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                diag.note(rationale_message);
                if let Some(replacement) = self.replacement {
                    diag.span_suggestion(
                        self.span,
                        remediation_message,
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation_message);
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MalformedCodePhaseComments: Explanatory phase-comment grammar
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
    #[doc = include_str!("README.md")]
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
        if matches!(kind, FnKind::Closure) || span.from_expansion() || span.is_build_generated(cx) {
            return;
        }
        for finding in self.analyzer.analyze_layout(cx, body).malformed {
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
                replacement: finding.replacement,
            }
            .emit(cx);
        }
    }
}
