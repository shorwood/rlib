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
// Violation: Oversized match arm diagnostic
// -----------------------------------------------------------------------------

/// Match arm whose authored implementation exceeds the configured line budget.
struct Violation {
    /// Arm body span used as the primary diagnostic location.
    span: Span,
    /// Analyzer-derived message containing the measured line count.
    primary_message: String,
    /// Analyzer-derived extraction guidance.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "inline implementation detail obscures the match's role as a readable table of alternatives",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            OVERSIZED_MATCH_ARMS,
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
// OversizedMatchArms
// -----------------------------------------------------------------------------

/// Late lint pass that limits authored code lines in one match arm.
struct OversizedMatchArms {
    /// Shared named-function analyzer configured for this lint family.
    analyzer: FunctionStructureAnalyzer,
}

impl OversizedMatchArms {
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
    /// Finds match arms whose authored code exceeds the configured line limit. Blank lines,
    /// comments, and the arm's outer braces are excluded from the count.
    ///
    /// ### Why is this bad?
    ///
    /// A match should reveal the alternatives and the operation selected by each one. Large inline
    /// branches bury that decision table beneath implementation detail, and phase comments cannot
    /// restore the lost overview.
    ///
    /// ```rust
    /// fn render(value: Option<u8>) {
    ///     match value {
    ///         Some(value) => {
    ///             let doubled = value * 2;
    ///             let text = doubled.to_string();
    ///             println!("{text}");
    ///         }
    ///         None => {}
    ///     }
    /// }
    /// ```
    ///
    /// Extract branch behavior into a helper whose name explains the selected operation.
    pub OVERSIZED_MATCH_ARMS,
    Warn,
    "rejects match arms beyond the configured source line limit",
    OversizedMatchArms::new()
}

impl<'tcx> LateLintPass<'tcx> for OversizedMatchArms {
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
            .oversized_match_arms
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
