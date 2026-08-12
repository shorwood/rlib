extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, PatKind, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::result_loss_analysis::{ResultContract, ResultLossAnalyzer};

// -----------------------------------------------------------------------------
// ResultDiscard: Explicit failure erasure syntax
// -----------------------------------------------------------------------------

/// Authored syntax that consumes a complete Result without inspecting either branch.
enum ResultDiscard {
    /// Wildcard `let` binding that suppresses the Result's must-use contract.
    WildcardBinding,
    /// Underscore-prefixed binding that advertises the Result will not be used normally.
    UnderscoreBinding,
    /// Explicit call to standard `drop` with the Result as its argument.
    DropCall,
}

impl ResultDiscard {
    /// Describes the concrete syntax responsible for erasing the Result.
    const fn description(&self) -> &'static str {
        match self {
            Self::WildcardBinding => "wildcard binding",
            Self::UnderscoreBinding => "underscore-prefixed binding",
            Self::DropCall => "explicit `drop` call",
        }
    }
}

// -----------------------------------------------------------------------------
// Violation: Discarded result context
// -----------------------------------------------------------------------------

/// Result disposal carrying the precise error information that becomes unreachable.
struct Violation {
    /// Complete discard expression or initializer used as the primary location.
    span: Span,
    /// Result value labeled as the source of the erased failure information.
    result_span: Span,
    /// Concrete error type callers can no longer inspect after the discard.
    error_type: String,
    /// Authored disposal form used to tailor the primary explanation.
    discard: ResultDiscard,
}

impl Violation {
    /// Builds a complete violation from one resolved Result contract.
    fn from_contract(
        span: Span,
        result_span: Span,
        contract: ResultContract<'_>,
        discard: ResultDiscard,
    ) -> Self {
        Self {
            span,
            result_span,
            error_type: contract.error_name(),
            discard,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this {} discards a `Result` without handling `{}`",
            self.discard.description(),
            self.error_type
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "discarding the complete value erases whether the operation failed and makes `{}` unavailable for recovery, reporting, or context",
            self.error_type
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "propagate the Result, inspect its error branch, or translate the failure at an explicit match boundary",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DISCARDED_RESULTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.result_span, "this Result is discarded in full");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DiscardedResults: Failure preservation policy
// -----------------------------------------------------------------------------

/// Finds authored Result values consumed without an explicit success or error policy.
struct DiscardedResults;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DISCARDED_RESULTS,
    Warn,
    "rejects Result values discarded without an explicit failure policy",
    DiscardedResults
}

impl LateLintPass<'_> for DiscardedResults {
    fn check_stmt(&mut self, cx: &LateContext<'_>, statement: &Stmt<'_>) {
        let Some(violation) = Self::binding_violation(cx, statement) else {
            return;
        };
        violation.emit(cx);
    }

    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let Some(violation) = Self::drop_violation(cx, expression) else {
            return;
        };
        violation.emit(cx);
    }
}

impl DiscardedResults {
    /// Classifies one authored wildcard or underscore-prefixed binding of a standard result.
    fn binding_violation(cx: &LateContext<'_>, statement: &Stmt<'_>) -> Option<Violation> {
        // Resolve the authored binding syntax and its initializer.
        if statement.span.from_expansion() {
            return None;
        }
        let StmtKind::Let(local) = statement.kind else {
            return None;
        };

        // Classify only bindings whose spelling explicitly advertises disposal.
        let discard = match local.pat.kind {
            PatKind::Wild => ResultDiscard::WildcardBinding,
            PatKind::Binding(_, _, ident, None) if ident.name.as_str().starts_with('_') => {
                ResultDiscard::UnderscoreBinding
            }
            _ => return None,
        };
        let initializer = local.init?;

        // Require the initializer itself to carry a standard result contract.
        let analyzer = ResultLossAnalyzer::for_context(cx);
        let contract = analyzer.contract(initializer)?;

        // Capture the authored discard and its precise failure context.
        Some(Violation::from_contract(
            statement.span,
            initializer.span,
            contract,
            discard,
        ))
    }

    /// Classifies one authored standard drop call consuming a result.
    fn drop_violation(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<Violation> {
        // Resolve only authored calls to standard drop with one result argument.
        if expression.span.from_expansion() {
            return None;
        }
        let rustc_hir::ExprKind::Call(_, [result]) = expression.kind else {
            return None;
        };

        // Require the consumed argument to carry a standard result contract.
        let analyzer = ResultLossAnalyzer::for_context(cx);
        let contract = analyzer.dropped_result(expression)?;

        // Capture the authored discard and its precise failure context.
        Some(Violation::from_contract(
            expression.span,
            result.span,
            contract,
            ResultDiscard::DropCall,
        ))
    }
}
