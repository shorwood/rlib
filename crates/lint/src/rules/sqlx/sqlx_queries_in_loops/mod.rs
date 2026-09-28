extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::{SqlxExprExt as _, is_query_execution};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Query execution in a loop
// -----------------------------------------------------------------------------

/// One database round trip nested inside an authored loop.
struct Violation {
    /// HIR owner receiving the lint.
    owner: rustc_hir::HirId,
    /// Authored execution call span.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("SQLx query is executed once per loop iteration")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "per-item database round trips multiply latency and can exhaust the connection pool under load",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "batch values, express the operation as one set-based query, join related data, or preload it before the loop",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_QUERIES_IN_LOOPS,
            self.owner,
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
// SqlxQueriesInLoops: Lint pass
// -----------------------------------------------------------------------------

/// Tracks loop nesting and detects `SQLx` executor calls within it.
#[derive(Default)]
struct SqlxQueriesInLoops {
    /// Number of loop expressions containing the current expression.
    depth: usize,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_QUERIES_IN_LOOPS,
    Warn,
    "rejects database round trips performed once per loop iteration",
    SqlxQueriesInLoops::default()
}

impl LateLintPass<'_> for SqlxQueriesInLoops {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Enter loops before visiting their body expressions.
        if matches!(expression.kind, ExprKind::Loop(..)) {
            self.depth += 1;
            return;
        }

        // Top-level and expansion-generated operations are outside authored loop policy.
        if self.depth == 0 || expression.span.from_expansion() {
            return;
        }

        // Non-SQLx expressions cannot perform a SQLx database round trip.
        let Some(call) = expression.sqlx_operation(cx) else {
            return;
        };

        // Query construction without execution does not create a round trip.
        if !is_query_execution(&call.name) {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span,
        }
        .emit(cx);
    }

    fn check_expr_post(&mut self, _cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Only loop exits change the tracked nesting depth.
        if !matches!(expression.kind, ExprKind::Loop(..)) {
            return;
        }
        self.depth = self.depth.saturating_sub(1);
    }
}
