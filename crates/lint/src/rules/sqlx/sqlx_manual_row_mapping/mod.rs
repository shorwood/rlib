extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind, HirId, StructTailExpr};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::{local_binding, operation};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
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

struct SqlxManualRowMapping;

struct RowAccessor<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    row: Option<HirId>,
}

impl<'tcx> Visitor<'tcx> for RowAccessor<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.row.is_some() {
            return;
        }
        if let Some(call) = operation(self.cx, expression)
            && matches!(call.name.as_str(), "get" | "try_get")
            && let Some(receiver) = call.receiver.and_then(local_binding)
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

impl<'tcx> LateLintPass<'tcx> for SqlxManualRowMapping {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        if expression.span.from_expansion() {
            return;
        }
        let ExprKind::Struct(_, fields, StructTailExpr::None) = expression.kind else {
            return;
        };
        if fields.len() < 2 {
            return;
        }
        let mut row = None;
        for field in fields {
            let mut accessor = RowAccessor { cx, row: None };
            accessor.visit_expr(field.expr);
            let Some(receiver) = accessor.row else {
                return;
            };
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
