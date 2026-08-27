extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::SqlxExprExt as _;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Unstructured AssertSqlSafe value
// -----------------------------------------------------------------------------

/// One unstructured runtime string asserted to be safe SQL.
struct Violation {
    /// HIR owner receiving the lint.
    owner: rustc_hir::HirId,
    /// Authored assertion call span.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("`AssertSqlSafe` wraps an unstructured runtime string")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a safety assertion does not prevent formatted or user-controlled values from becoming executable SQL",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "bind data values and represent dynamic identifiers or keywords with an exhaustively rendered domain type",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_UNSTRUCTURED_ASSERT_SQL_SAFE,
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
// SqlxUnstructuredAssertSqlSafe: Lint pass
// -----------------------------------------------------------------------------

/// Detects safety assertions over primitive runtime string carriers.
struct SqlxUnstructuredAssertSqlSafe;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_UNSTRUCTURED_ASSERT_SQL_SAFE,
    Warn,
    "rejects AssertSqlSafe around unstructured runtime string values",
    SqlxUnstructuredAssertSqlSafe
}

impl LateLintPass<'_> for SqlxUnstructuredAssertSqlSafe {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Expansion internals are owned by the originating macro.
        if expression.span.from_expansion() {
            return;
        }

        // Non-SQLx expressions cannot construct AssertSqlSafe.
        let Some(call) = expression.sqlx_operation(cx) else {
            return;
        };

        // Calls without a value cannot expose unstructured SQL input.
        let Some(value) = call.arguments.first() else {
            return;
        };

        // Static text, domain types, and unrelated APIs do not erase SQL structure.
        if call.name != "AssertSqlSafe"
            || value.static_string().is_some()
            || !value.is_unstructured_value(cx)
        {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span,
        }
        .emit(cx);
    }
}
