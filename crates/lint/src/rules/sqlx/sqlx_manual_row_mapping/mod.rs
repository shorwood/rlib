extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind, HirId, StructTailExpr};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::SqlxExprExt as _;
use crate::utils::diagnostic::LateViolation;

/// Minimum fields that establish a repetitive row-to-struct mapping.
const MINIMUM_MAPPED_FIELDS: usize = 2;

// -----------------------------------------------------------------------------
// Violation: Manual row mapping
// -----------------------------------------------------------------------------

/// One struct literal populated through repetitive access to the same `SQLx` row.
struct Violation {
    /// HIR owner receiving the lint.
    owner: rustc_hir::HirId,
    /// Authored struct expression span.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("struct is populated by repetitive access to one SQLx row")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "mechanical field-by-field decoding duplicates the schema mapping and its error handling",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive `FromRow` and use `query_as`, or use `query_as!` for compile-time checked named output",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_MANUAL_ROW_MAPPING,
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
// SqlxManualRowMapping: Lint pass
// -----------------------------------------------------------------------------

/// Detects mechanical multi-field struct construction from one `SQLx` row.
struct SqlxManualRowMapping;

impl<'tcx> LateLintPass<'tcx> for SqlxManualRowMapping {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Expansion internals are owned by the originating macro.
        if expression.span.from_expansion() {
            return;
        }

        // Only complete struct literals represent a direct row mapping.
        let ExprKind::Struct(_, fields, StructTailExpr::None) = expression.kind else {
            return;
        };

        // One accessed field does not establish repetitive mapping boilerplate.
        if fields.len() < MINIMUM_MAPPED_FIELDS {
            return;
        }
        let mut row = None;
        for field in fields {
            let mut accessor = RowAccessor { cx, row: None };
            accessor.visit_expr(field.expr);

            // Every field must read directly from a row binding.
            let Some(receiver) = accessor.row else {
                return;
            };

            // Reads from multiple rows do not form one derivable mapping contract.
            if row.is_some_and(|row| row != receiver) {
                return;
            }
            row = Some(receiver);
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span,
        }
        .emit(cx);
    }
}

// -----------------------------------------------------------------------------
// RowAccessor: Field-local row lookup
// -----------------------------------------------------------------------------

/// Finds the first `SQLx` Row accessor used inside one field expression.
struct RowAccessor<'cx, 'tcx> {
    /// Compiler context used to resolve `SQLx` operations.
    cx: &'cx LateContext<'tcx>,
    /// Local row binding found in the field expression.
    row: Option<HirId>,
}

impl<'tcx> Visitor<'tcx> for RowAccessor<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // The first row accessor determines this field's mapping source.
        if self.row.is_some() {
            return;
        }

        // A resolved Row accessor completes the field-local search.
        if let Some(call) = expression.sqlx_operation(self.cx)
            && matches!(call.name.as_str(), "get" | "try_get")
            && let Some(receiver) = call
                .receiver
                .and_then(super::utils::SqlxExprExt::local_binding)
        {
            self.row = Some(receiver);
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_MANUAL_ROW_MAPPING,
    Warn,
    "replaces mechanical SQLx row extraction with structured query mapping",
    SqlxManualRowMapping
}
