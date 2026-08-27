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
// Violation: Unstructured builder fragment
// -----------------------------------------------------------------------------

/// One primitive runtime value appended directly to QueryBuilder SQL text.
struct Violation {
    /// HIR owner receiving the lint.
    owner: rustc_hir::HirId,
    /// Authored fragment span.
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

// -----------------------------------------------------------------------------
// SqlxUnstructuredQueryBuilderFragments: Lint pass
// -----------------------------------------------------------------------------

/// Detects unstructured runtime fragments passed to QueryBuilder push operations.
struct SqlxUnstructuredQueryBuilderFragments;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_UNSTRUCTURED_QUERY_BUILDER_FRAGMENTS,
    Warn,
    "rejects unstructured runtime values appended directly to QueryBuilder SQL",
    SqlxUnstructuredQueryBuilderFragments
}

impl SqlxUnstructuredQueryBuilderFragments {
    /// Unwraps AssertSqlSafe so the underlying fragment representation can be classified.
    fn underlying_fragment<'hir>(
        cx: &LateContext<'_>,
        fragment: &'hir Expr<'hir>,
    ) -> &'hir Expr<'hir> {
        fragment
            .sqlx_operation(cx)
            .filter(|wrapper| wrapper.name == "AssertSqlSafe")
            .and_then(|wrapper| wrapper.arguments.first())
            .unwrap_or(fragment)
    }
}

impl LateLintPass<'_> for SqlxUnstructuredQueryBuilderFragments {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Expansion internals are owned by the originating macro.
        if expression.span.from_expansion() {
            return;
        }

        // Non-SQLx expressions cannot mutate QueryBuilder SQL.
        let Some(call) = expression.sqlx_operation(cx) else {
            return;
        };

        // Calls without a fragment cannot append unstructured SQL.
        let Some(fragment) = call.arguments.first() else {
            return;
        };
        let fragment = Self::underlying_fragment(cx, fragment);

        // Bind operations, static text, and domain types retain a structured boundary.
        if !matches!(call.name.as_str(), "push" | "push_unseparated")
            || fragment.static_string().is_some()
            || !fragment.is_unstructured_value(cx)
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
