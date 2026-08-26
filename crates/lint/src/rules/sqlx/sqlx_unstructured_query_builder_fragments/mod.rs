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
        Cow::Borrowed("runtime value is appended directly to SQL text")
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "`QueryBuilder::push` performs no sanitization, so unstructured values can change the query grammar",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use `push_bind` for data or an exhaustively rendered domain type for structural SQL fragments",
        )
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_UNSTRUCTURED_QUERY_BUILDER_FRAGMENTS,
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

struct SqlxUnstructuredQueryBuilderFragments;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_UNSTRUCTURED_QUERY_BUILDER_FRAGMENTS,
    Warn,
    "rejects unstructured runtime values appended directly to QueryBuilder SQL",
    SqlxUnstructuredQueryBuilderFragments
}

impl LateLintPass<'_> for SqlxUnstructuredQueryBuilderFragments {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        if expression.span.from_expansion() {
            return;
        }
        let Some(call) = operation(cx, expression) else {
            return;
        };
        let Some(fragment) = call.arguments.first() else {
            return;
        };
        let fragment = operation(cx, fragment)
            .filter(|wrapper| wrapper.name == "AssertSqlSafe")
            .and_then(|wrapper| wrapper.arguments.first())
            .unwrap_or(fragment);
        if !matches!(call.name.as_str(), "push" | "push_unseparated")
            || static_string(fragment).is_some()
            || !is_unstructured_value(cx, fragment)
        {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: fragment.span,
        }
        .emit(cx);
    }
}
