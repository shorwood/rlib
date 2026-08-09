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
    /// Explicit call to standard `drop` with the Result as its argument.
    DropCall,
}

impl ResultDiscard {
    /// Describes the concrete syntax responsible for erasing the Result.
    const fn description(&self) -> &'static str {
        match self {
            Self::WildcardBinding => "wildcard binding",
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
    /// ### What it does
    ///
    /// Finds standard `Result` values deliberately consumed through a wildcard `let` binding or
    /// an explicit call to `drop`. Ordinary unused Results remain covered by Rust's
    /// `unused_must_use` lint, while non-Result values and macro-generated code remain valid.
    ///
    /// ### Why is this bad?
    ///
    /// Both forms silence the type's must-use contract without stating whether failure should be
    /// propagated, reported, translated, or intentionally ignored. This is especially easy for
    /// generated glue code to introduce when only the happy-path side effect appears relevant.
    /// Once the complete value is gone, later code cannot recover the concrete error or attach the
    /// context needed to diagnose the failed operation.
    ///
    /// ```rust
    /// # fn persist() -> Result<(), std::io::Error> { Ok(()) }
    /// let _ = persist();
    /// drop(persist());
    /// ```
    ///
    /// Keep the failure visible at a deliberate policy boundary:
    ///
    /// ```rust
    /// # fn persist() -> Result<(), std::io::Error> { Ok(()) }
    /// # fn report(_: std::io::Error) {}
    /// if let Err(error) = persist() {
    ///     report(error);
    /// }
    /// ```
    ///
    /// This lint intentionally provides no automatic fix. Propagation with `?`, contextual error
    /// translation, retry, logging, and deliberate rejection are different contracts that require
    /// domain judgment.
    pub DISCARDED_RESULTS,
    Warn,
    "rejects Result values discarded without an explicit failure policy",
    DiscardedResults
}

impl LateLintPass<'_> for DiscardedResults {
    fn check_stmt(&mut self, cx: &LateContext<'_>, statement: &Stmt<'_>) {
        let Some(violation) = Self::wildcard_violation(cx, statement) else {
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
    /// Classifies one authored wildcard binding of a standard result.
    fn wildcard_violation(cx: &LateContext<'_>, statement: &Stmt<'_>) -> Option<Violation> {
        // Resolve the authored wildcard syntax and its initializer.
        if statement.span.from_expansion() {
            return None;
        }
        let StmtKind::Let(local) = statement.kind else {
            return None;
        };
        let PatKind::Wild = local.pat.kind else {
            return None;
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
            ResultDiscard::WildcardBinding,
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
