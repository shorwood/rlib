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

use super::utils::{SqlxExprExt as _, is_query_execution};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Needless pool acquisition
// -----------------------------------------------------------------------------

/// One connection acquired solely to execute a query supported by the pool.
struct Violation {
    /// Authored acquisition statement span.
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

// -----------------------------------------------------------------------------
// AcquireFinder: Pool acquisition lookup
// -----------------------------------------------------------------------------

/// Finds a SQLx Pool acquisition inside an initializer expression.
struct AcquireFinder<'cx, 'tcx> {
    /// Compiler context used to resolve SQLx calls and types.
    cx: &'cx LateContext<'tcx>,
    /// Whether a qualifying acquisition has been found.
    is_found: bool,
}

impl AcquireFinder<'_, '_> {
    /// Returns whether an expression has SQLx's Pool type.
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
        // Discovery is complete after the first qualifying acquisition.
        if self.is_found {
            return;
        }

        // A qualifying acquisition completes the initializer-local search.
        if let Some(call) = expression.sqlx_operation(self.cx)
            && call.name == "acquire"
            && call.receiver.is_some_and(|receiver| self.is_pool(receiver))
        {
            self.is_found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

// -----------------------------------------------------------------------------
// LocalUseFinder: Binding reference lookup
// -----------------------------------------------------------------------------

/// Finds a reference to one local connection binding.
struct LocalUseFinder {
    /// Local binding being searched for.
    binding: HirId,
    /// Whether the binding has been referenced.
    is_found: bool,
}

impl<'tcx> Visitor<'tcx> for LocalUseFinder {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // A matching local path completes the expression-local search.
        if let ExprKind::Path(QPath::Resolved(_, path)) = expression.kind
            && matches!(path.res, Res::Local(binding) if binding == self.binding)
        {
            self.is_found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// ConnectionUse: Acquired connection lifecycle
// -----------------------------------------------------------------------------

/// Usage facts collected for one connection acquired from a pool.
struct ConnectionUse {
    /// Acquisition statement span.
    span: Span,
    /// Number of references to the local binding.
    uses: usize,
    /// Whether the binding is passed to a query execution operation.
    is_query_executor: bool,
}

// -----------------------------------------------------------------------------
// PoolAnalysis: Function-local connection analysis
// -----------------------------------------------------------------------------

/// Tracks acquired connections and classifies their uses within one function body.
struct PoolAnalysis<'cx, 'tcx> {
    /// Compiler context used to resolve SQLx operations.
    cx: &'cx LateContext<'tcx>,
    /// Connection usage indexed by local binding.
    connections: HashMap<HirId, ConnectionUse>,
}

impl<'tcx> PoolAnalysis<'_, 'tcx> {
    /// Returns whether an expression references the selected local binding.
    fn contains(expression: &'tcx Expr<'tcx>, binding: HirId) -> bool {
        let mut finder = LocalUseFinder {
            binding,
            is_found: false,
        };
        finder.visit_expr(expression);
        finder.is_found
    }

    /// Returns whether an expression contains a SQLx Pool acquisition.
    fn is_acquired_from_pool(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = AcquireFinder {
            cx: self.cx,
            is_found: false,
        };
        finder.visit_expr(expression);
        finder.is_found
    }
}

impl<'tcx> Visitor<'tcx> for PoolAnalysis<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = statement.kind
            && let PatKind::Binding(_, binding, _, None) = local.pat.kind
            && let Some(initializer) = local.init
            && self.is_acquired_from_pool(initializer)
        {
            self.connections.insert(
                binding,
                ConnectionUse {
                    span: statement.span,
                    uses: 0,
                    is_query_executor: false,
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

        if let Some(call) = expression.sqlx_operation(self.cx)
            && is_query_execution(&call.name)
        {
            let bindings: Vec<_> = self.connections.keys().copied().collect();
            for binding in bindings {
                if !call
                    .arguments
                    .iter()
                    .any(|argument| Self::contains(argument, binding))
                {
                    continue;
                }
                self.connections
                    .get_mut(&binding)
                    .expect("known binding")
                    .is_query_executor = true;
            }
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

// -----------------------------------------------------------------------------
// SqlxNeedlessPoolAcquisition: Lint pass
// -----------------------------------------------------------------------------

/// Detects connections whose only purpose can be served directly by their pool.
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
            if connection.uses != 1 || !connection.is_query_executor {
                continue;
            }
            Violation {
                span: connection.span,
            }
            .emit(cx);
        }
    }
}
