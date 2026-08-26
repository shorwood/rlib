extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::{operation, static_string};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("static SQL is passed through a runtime-checked query API")
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a complete static query can use SQLx's compile-time database, bind-count, and Rust type validation",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use the matching `query!`, `query_as!`, `query_scalar!`, or file-backed checked macro",
        )
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_STATIC_QUERIES_WITHOUT_COMPILE_TIME_CHECKS,
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

struct SqlxStaticQueriesWithoutCompileTimeChecks;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_STATIC_QUERIES_WITHOUT_COMPILE_TIME_CHECKS,
    Warn,
    "requires statically authored prepared SQL to use SQLx checked macros",
    SqlxStaticQueriesWithoutCompileTimeChecks
}

impl LateLintPass<'_> for SqlxStaticQueriesWithoutCompileTimeChecks {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        if expression.span.from_expansion() {
            return;
        }
        let Some(call) = operation(cx, expression) else {
            return;
        };
        if !matches!(
            call.name.as_str(),
            "query"
                | "query_with"
                | "query_as"
                | "query_as_with"
                | "query_scalar"
                | "query_scalar_with"
        ) || call.arguments.first().and_then(static_string).is_none()
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
