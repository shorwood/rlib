extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::{operation, root_local};
use crate::utils::diagnostic::LateViolation;

enum ReuseKind {
    Build,
    Mutation,
}

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    kind: ReuseKind,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(match self.kind {
            ReuseKind::Build => "QueryBuilder is built again without being reset",
            ReuseKind::Mutation => "QueryBuilder is modified after build without being reset",
        })
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "SQLx requires `reset()` before any reuse after `build*()` and otherwise panics",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("call `reset()` after the built query is finished, or create a fresh builder")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_QUERY_BUILDER_REUSED_WITHOUT_RESET,
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
struct SqlxQueryBuilderReusedWithoutReset {
    owner: Option<LocalDefId>,
    built: HashSet<rustc_hir::HirId>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_QUERY_BUILDER_REUSED_WITHOUT_RESET,
    Warn,
    "requires QueryBuilder reset before reuse after build",
    SqlxQueryBuilderReusedWithoutReset::default()
}

impl LateLintPass<'_> for SqlxQueryBuilderReusedWithoutReset {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        if self.owner != Some(owner) {
            self.owner = Some(owner);
            self.built.clear();
        }
        if expression.span.from_expansion() {
            return;
        }
        let Some(call) = operation(cx, expression) else {
            return;
        };
        let Some(receiver) = call.receiver else {
            return;
        };
        let Some(binding) = root_local(receiver) else {
            return;
        };
        if call.name == "reset" {
            self.built.remove(&binding);
            return;
        }
        if matches!(
            call.name.as_str(),
            "build" | "build_query_as" | "build_query_scalar"
        ) {
            if self.built.contains(&binding) {
                Violation {
                    owner: expression.hir_id,
                    span: expression.span,
                    kind: ReuseKind::Build,
                }
                .emit(cx);
            }
            self.built.insert(binding);
        } else if self.built.contains(&binding)
            && matches!(
                call.name.as_str(),
                "push" | "push_bind" | "push_values" | "push_tuples" | "separated"
            )
        {
            Violation {
                owner: expression.hir_id,
                span: expression.span,
                kind: ReuseKind::Mutation,
            }
            .emit(cx);
        }
    }
}
