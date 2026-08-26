extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::{is_unstructured_value, operation, static_string};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
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
        if expression.span.from_expansion() {
            return;
        }
        let Some(call) = operation(cx, expression) else {
            return;
        };
        let Some(value) = call.arguments.first() else {
            return;
        };
        if call.name != "AssertSqlSafe"
            || static_string(value).is_some()
            || !is_unstructured_value(cx, value)
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
