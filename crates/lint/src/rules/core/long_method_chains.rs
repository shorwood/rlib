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
// Violation: Long method chain diagnostic
// -----------------------------------------------------------------------------

/// Fluent expression whose resolved call count exceeds the configured limit.
struct Violation {
    /// Complete chain span used as the primary diagnostic location.
    span: Span,
    /// Analyzer-derived message containing the measured call count.
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
            "the chain hides intermediate domain states and failure boundaries inside one dense expression",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            LONG_METHOD_CHAINS,
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
// LongMethodChains
// -----------------------------------------------------------------------------

/// Late lint pass that limits calls in one fluent method chain.
struct LongMethodChains {
    /// Shared named-function analyzer configured for this lint family.
    analyzer: FunctionStructureAnalyzer,
}

impl LongMethodChains {
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
    /// Finds maximal expressions containing more consecutive method calls than the configured
    /// limit. Associated-function calls do not count, and field access or `?` starts a new chain.
    ///
    /// ### Why is this bad?
    ///
    /// Long fluent expressions hide meaningful transformations behind punctuation and leave no
    /// vocabulary for intermediate states. Named bindings make the data flow inspectable and give
    /// later operations a concept to refer to.
    ///
    /// ```rust
    /// fn names(values: &[String]) -> Vec<String> {
    ///     values.iter().filter(|value| !value.is_empty()).cloned().collect()
    /// }
    /// ```
    ///
    /// Break the expression where an intermediate state has a useful name:
    ///
    /// ```rust
    /// fn names(values: &[String]) -> Vec<String> {
    ///     let populated = values.iter().filter(|value| !value.is_empty());
    ///     populated.cloned().collect()
    /// }
    /// ```
    pub LONG_METHOD_CHAINS,
    Warn,
    "rejects method chains beyond the configured call limit",
    LongMethodChains::new()
}

impl<'tcx> LateLintPass<'tcx> for LongMethodChains {
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
            .long_method_chains
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
