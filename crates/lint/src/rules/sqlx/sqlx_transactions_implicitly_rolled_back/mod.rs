extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, HirId, PatKind, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::SqlxExprExt as _;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Implicit transaction rollback
// -----------------------------------------------------------------------------

/// One transaction binding still live when its function body ends.
struct Violation {
    /// Authored transaction binding span.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("SQLx transaction can leave this function without explicit finalization")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "dropping a live transaction rolls it back implicitly, hiding the function's commit policy in destructor behavior",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "explicitly call `commit()` or `rollback()` on every intended transaction path",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SQLX_TRANSACTIONS_IMPLICITLY_ROLLED_BACK,
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
// BeginFinder: Transaction creation lookup
// -----------------------------------------------------------------------------

/// Finds a SQLx transaction begin operation inside an initializer.
struct BeginFinder<'cx, 'tcx> {
    /// Compiler context used to resolve SQLx calls.
    cx: &'cx LateContext<'tcx>,
    /// Whether a begin operation has been found.
    is_found: bool,
}

impl<'tcx> Visitor<'tcx> for BeginFinder<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Discovery is complete after the first begin operation.
        if self.is_found {
            return;
        }

        // A begin operation completes the initializer-local search.
        if expression
            .sqlx_operation(self.cx)
            .is_some_and(|call| call.name == "begin")
        {
            self.is_found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

// -----------------------------------------------------------------------------
// TransactionAnalysis: Function-local transaction lifecycle
// -----------------------------------------------------------------------------

/// Tracks transaction bindings until an explicit commit or rollback.
struct TransactionAnalysis<'cx, 'tcx> {
    /// Compiler context used to resolve SQLx operations.
    cx: &'cx LateContext<'tcx>,
    /// Live transaction bindings and their declaration spans.
    live: HashMap<HirId, Span>,
}

impl<'tcx> TransactionAnalysis<'_, 'tcx> {
    /// Returns whether an initializer contains a SQLx begin operation.
    fn is_begin_transaction(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = BeginFinder {
            cx: self.cx,
            is_found: false,
        };
        finder.visit_expr(expression);
        finder.is_found
    }
}

impl<'tcx> Visitor<'tcx> for TransactionAnalysis<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = statement.kind
            && let PatKind::Binding(_, binding, _, None) = local.pat.kind
            && let Some(initializer) = local.init
            && self.is_begin_transaction(initializer)
        {
            self.live.insert(binding, statement.span);
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let Some(call) = expression.sqlx_operation(self.cx)
            && matches!(call.name.as_str(), "commit" | "rollback")
            && let Some(binding) = call.receiver.and_then(|receiver| receiver.root_local())
        {
            self.live.remove(&binding);
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

// -----------------------------------------------------------------------------
// SqlxTransactionsImplicitlyRolledBack: Lint pass
// -----------------------------------------------------------------------------

/// Detects transaction bindings that rely on drop for rollback policy.
struct SqlxTransactionsImplicitlyRolledBack;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_TRANSACTIONS_IMPLICITLY_ROLLED_BACK,
    Warn,
    "requires explicit SQLx transaction finalization",
    SqlxTransactionsImplicitlyRolledBack
}

impl<'tcx> LateLintPass<'tcx> for SqlxTransactionsImplicitlyRolledBack {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx rustc_hir::FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        _: LocalDefId,
    ) {
        let mut analysis = TransactionAnalysis {
            cx,
            live: HashMap::new(),
        };
        analysis.visit_body(body);
        for span in analysis.live.into_values() {
            Violation { span }.emit(cx);
        }
    }
}
