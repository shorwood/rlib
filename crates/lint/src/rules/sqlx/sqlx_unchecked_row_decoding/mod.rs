extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::operation;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    operation: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`Row::{}` skips SQL type compatibility checking",
            self.operation
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "unchecked decoding can reinterpret a schema mismatch as an unrelated Rust value or late decoding failure",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use `try_get` or a checked query mapping and make any custom conversion explicit",
        )
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_UNCHECKED_ROW_DECODING,
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

struct SqlxUncheckedRowDecoding;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_UNCHECKED_ROW_DECODING,
    Warn,
    "rejects SQLx row decoding that skips type compatibility checks",
    SqlxUncheckedRowDecoding
}

impl LateLintPass<'_> for SqlxUncheckedRowDecoding {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        if expression.span.from_expansion() {
            return;
        }
        let Some(call) = operation(cx, expression) else {
            return;
        };
        if !matches!(call.name.as_str(), "get_unchecked" | "try_get_unchecked") {
            return;
        }
        let Some(trait_id) = cx.tcx.trait_of_assoc(call.definition) else {
            return;
        };
        if cx.tcx.item_name(trait_id).as_str() != "Row" {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span,
            operation: call.name,
        }
        .emit(cx);
    }
}
