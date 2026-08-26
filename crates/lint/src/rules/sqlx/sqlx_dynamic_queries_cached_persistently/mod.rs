extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_ast::LitKind;
use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::{is_query_execution, operation, root_local};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("runtime-varying query shape remains persistently cached")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "each distinct bind cardinality can occupy another prepared-statement cache entry on every connection",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("call `.persistent(false)` on the built query before executing it")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_DYNAMIC_QUERIES_CACHED_PERSISTENTLY,
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
struct SqlxDynamicQueriesCachedPersistently {
    varying_builders: HashMap<LocalDefId, HashSet<rustc_hir::HirId>>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_DYNAMIC_QUERIES_CACHED_PERSISTENTLY,
    Warn,
    "requires runtime-varying QueryBuilder shapes to disable persistent statement caching",
    SqlxDynamicQueriesCachedPersistently::default()
}

impl SqlxDynamicQueriesCachedPersistently {
    fn has_false_persistent(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let Some(call) = operation(cx, expression) else {
            return false;
        };
        if call.name == "persistent"
            && matches!(call.arguments, [argument] if matches!(argument.kind, ExprKind::Lit(literal) if matches!(literal.node, LitKind::Bool(false))))
        {
            return true;
        }
        call.receiver
            .is_some_and(|receiver| Self::has_false_persistent(cx, receiver))
    }

    fn has_varying_builder_shape(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let Some(call) = operation(cx, expression) else {
            return false;
        };
        if matches!(call.name.as_str(), "push_values" | "push_tuples") {
            return true;
        }
        call.receiver
            .is_some_and(|receiver| Self::has_varying_builder_shape(cx, receiver))
    }
}

impl LateLintPass<'_> for SqlxDynamicQueriesCachedPersistently {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        if expression.span.from_expansion() {
            return;
        }
        let Some(call) = operation(cx, expression) else {
            return;
        };
        if matches!(call.name.as_str(), "push_values" | "push_tuples")
            && let Some(binding) = call.receiver.and_then(root_local)
        {
            self.varying_builders
                .entry(owner)
                .or_default()
                .insert(binding);
            return;
        }
        let Some(query) = call.receiver else {
            return;
        };
        let recorded_shape = root_local(query).is_some_and(|binding| {
            self.varying_builders
                .get(&owner)
                .is_some_and(|builders| builders.contains(&binding))
        });
        if !is_query_execution(&call.name)
            || !(recorded_shape || Self::has_varying_builder_shape(cx, query))
            || Self::has_false_persistent(cx, query)
        {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: query.span,
        }
        .emit(cx);
    }
}
