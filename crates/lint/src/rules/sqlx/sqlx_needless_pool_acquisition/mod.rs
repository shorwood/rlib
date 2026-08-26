extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId, PatKind, QPath, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::{is_query_execution, operation};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("pool connection is acquired only to execute one SQLx query")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "SQLx pools implement `Executor` directly, so the extra acquisition adds lifecycle state without changing query ownership",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("execute the query against `&pool` directly")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SQLX_NEEDLESS_POOL_ACQUISITION,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct AcquireFinder<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    found: bool,
}

impl AcquireFinder<'_, '_> {
    fn is_pool(&self, expression: &Expr<'_>) -> bool {
        let ty = self
            .cx
            .typeck_results()
            .expr_ty_adjusted(expression)
            .peel_refs();
        matches!(ty.kind(), ty::Adt(definition, _) if {
            self.cx.tcx.item_name(definition.did()).as_str() == "Pool"
                && self.cx.tcx.crate_name(definition.did().krate).as_str() == "sqlx_core"
        })
    }
}

impl<'tcx> Visitor<'tcx> for AcquireFinder<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.found {
            return;
        }
        if let Some(call) = operation(self.cx, expression)
            && call.name == "acquire"
            && call.receiver.is_some_and(|receiver| self.is_pool(receiver))
        {
            self.found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

struct LocalUseFinder {
    binding: HirId,
    found: bool,
}

impl<'tcx> Visitor<'tcx> for LocalUseFinder {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(QPath::Resolved(_, path)) = expression.kind
            && matches!(path.res, Res::Local(binding) if binding == self.binding)
        {
            self.found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

struct ConnectionUse {
    span: Span,
    uses: usize,
    query_executor: bool,
}

struct PoolAnalysis<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    connections: HashMap<HirId, ConnectionUse>,
}

impl<'tcx> PoolAnalysis<'_, 'tcx> {
    fn acquired_from_pool(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = AcquireFinder {
            cx: self.cx,
            found: false,
        };
        finder.visit_expr(expression);
        finder.found
    }

    fn contains(expression: &'tcx Expr<'tcx>, binding: HirId) -> bool {
        let mut finder = LocalUseFinder {
            binding,
            found: false,
        };
        finder.visit_expr(expression);
        finder.found
    }
}

impl<'tcx> Visitor<'tcx> for PoolAnalysis<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = statement.kind
            && let PatKind::Binding(_, binding, _, None) = local.pat.kind
            && let Some(initializer) = local.init
            && self.acquired_from_pool(initializer)
        {
            self.connections.insert(
                binding,
                ConnectionUse {
                    span: statement.span,
                    uses: 0,
                    query_executor: false,
                },
            );
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        let referenced = if let ExprKind::Path(QPath::Resolved(_, path)) = expression.kind {
            match path.res {
                Res::Local(binding) if self.connections.contains_key(&binding) => Some(binding),
                _ => None,
            }
        } else {
            None
        };
        if let Some(binding) = referenced {
            self.connections
                .get_mut(&binding)
                .expect("known binding")
                .uses += 1;
        }

        if let Some(call) = operation(self.cx, expression)
            && is_query_execution(&call.name)
        {
            let bindings: Vec<_> = self.connections.keys().copied().collect();
            for binding in bindings {
                if call
                    .arguments
                    .iter()
                    .any(|argument| Self::contains(argument, binding))
                {
                    self.connections
                        .get_mut(&binding)
                        .expect("known binding")
                        .query_executor = true;
                }
            }
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

struct SqlxNeedlessPoolAcquisition;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_NEEDLESS_POOL_ACQUISITION,
    Warn,
    "rejects acquiring a pool connection solely for one query",
    SqlxNeedlessPoolAcquisition
}

impl<'tcx> LateLintPass<'tcx> for SqlxNeedlessPoolAcquisition {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx rustc_hir::FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        _: LocalDefId,
    ) {
        let mut analysis = PoolAnalysis {
            cx,
            connections: HashMap::new(),
        };
        analysis.visit_body(body);
        for connection in analysis.connections.into_values() {
            if connection.uses == 1 && connection.query_executor {
                Violation {
                    span: connection.span,
                }
                .emit(cx);
            }
        }
    }
}
