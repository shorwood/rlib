extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::BTreeSet;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Pos, Span};

use super::utils::sqlx_macro;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    macro_name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}!` skips SQLx's Rust type checks",
            self.macro_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "unchecked query macros weaken the compile-time contract between database columns, bind parameters, and Rust types",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use the corresponding checked SQLx query macro and express exceptional mappings with supported type overrides",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_UNCHECKED_QUERY_MACROS,
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

#[derive(Default)]
struct SqlxUncheckedQueryMacros {
    seen: BTreeSet<(u32, u32)>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_UNCHECKED_QUERY_MACROS,
    Warn,
    "rejects SQLx query macros that skip Rust type checking",
    SqlxUncheckedQueryMacros::default()
}

impl LateLintPass<'_> for SqlxUncheckedQueryMacros {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let Some((name, call_site)) = sqlx_macro(cx, expression.span) else {
            return;
        };
        if !name.contains("unchecked")
            || !self
                .seen
                .insert((call_site.lo().to_u32(), call_site.hi().to_u32()))
        {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: call_site,
            macro_name: name,
        }
        .emit(cx);
    }
}
