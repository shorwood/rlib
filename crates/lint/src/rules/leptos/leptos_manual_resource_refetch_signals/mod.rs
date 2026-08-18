extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind, Node, PatKind, StmtKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::symbol::sym;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Manual resource refetch signal diagnostic
// -----------------------------------------------------------------------------

/// Discarded reactive read used only to create a resource dependency.
struct Violation {
    /// Read expression used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored discarded read highlighted by the diagnostic.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("discarded signal value is used as a resource refetch trigger")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a signal whose value is ignored hides an imperative reload behind a fake data dependency",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "remove the trigger signal and call `LocalResource::refetch()` after the mutation",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MANUAL_RESOURCE_REFETCH_SIGNALS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this tracked value is immediately discarded");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosManualResourceRefetchSignals: Resource dependency policy
// -----------------------------------------------------------------------------

/// Late lint pass that replaces dummy refresh signals with the resource refetch operation.
struct LeptosManualResourceRefetchSignals;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MANUAL_RESOURCE_REFETCH_SIGNALS,
    Warn,
    "rejects discarded signal reads used to refetch a Leptos local resource",
    LeptosManualResourceRefetchSignals
}

impl LeptosManualResourceRefetchSignals {
    /// Returns whether the expression resolves to a tracked reactive `get` operation.
    fn is_reactive_get(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // Only method calls can resolve to the tracked reactive get operation.
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return false;
        };

        // Reactive get accepts no explicit method arguments.
        if !arguments.is_empty() {
            return false;
        }
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Unresolved methods cannot prove the reactive get trait contract.
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        cx.tcx.crate_name(method.krate).as_str() == "reactive_graph"
            && cx.tcx.item_name(method).as_str() == "get"
            && cx
                .tcx
                .trait_of_assoc(method)
                .is_some_and(|trait_id| cx.tcx.item_name(trait_id).as_str() == "Get")
    }

    /// Returns whether this expression is a complete semicolon-terminated statement.
    fn is_discarded(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let mut discarded = expression.hir_id;
        for (_, node) in cx.tcx.hir_parent_iter(expression.hir_id) {
            match node {
                Node::Expr(parent) => {
                    // Discard wrappers must be one-argument calls around the tracked value.
                    let ExprKind::Call(callee, [argument]) = parent.kind else {
                        return false;
                    };

                    // Indirect callees cannot prove the standard drop wrapper.
                    let ExprKind::Path(path) = callee.kind else {
                        return false;
                    };

                    // Unresolved paths cannot identify the standard drop function.
                    let Res::Def(_, definition) = cx.qpath_res(&path, callee.hir_id) else {
                        return false;
                    };

                    // Only dropping the current value preserves the discarded-read chain.
                    if argument.hir_id != discarded
                        || !cx.tcx.is_diagnostic_item(sym::mem_drop, definition)
                    {
                        return false;
                    }
                    discarded = parent.hir_id;
                }
                // A statement determines whether the complete wrapped value is discarded.
                Node::Stmt(statement) => {
                    return match statement.kind {
                        StmtKind::Semi(value) => value.hir_id == discarded,
                        StmtKind::Let(local) => {
                            matches!(local.pat.kind, PatKind::Wild)
                                && local.init.is_some_and(|value| value.hir_id == discarded)
                        }
                        StmtKind::Expr(_) | StmtKind::Item(_) => false,
                    };
                }
                // A lowered wildcard let statement also discards its initializer.
                Node::LetStmt(local) => {
                    return matches!(local.pat.kind, PatKind::Wild)
                        && local.init.is_some_and(|value| value.hir_id == discarded);
                }
                // Other parent kinds mean the value participates in a larger expression.
                _ => return false,
            }
        }
        false
    }

    /// Returns whether the call resolves to `leptos_server::LocalResource::new`.
    fn is_local_resource_new(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // Only calls can construct a local resource.
        let ExprKind::Call(callee, _) = expression.kind else {
            return false;
        };

        // Indirect callees cannot identify the local-resource constructor.
        let ExprKind::Path(path) = callee.kind else {
            return false;
        };

        // Unresolved constructor paths cannot prove an associated resource method.
        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return false;
        };

        // Free functions cannot be the associated local-resource constructor.
        let Some(implementation) = cx.tcx.impl_of_assoc(method) else {
            return false;
        };

        // Implementations without a nominal self type cannot construct a local resource.
        let Some(definition) = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()
        else {
            return false;
        };
        cx.tcx.crate_name(method.krate).as_str() == "leptos_server"
            && cx.tcx.item_name(method).as_str() == "new"
            && matches!(
                cx.tcx.item_name(definition.did()).as_str(),
                "LocalResource" | "ArcLocalResource"
            )
    }

    /// Proves the discarded read belongs directly to the local resource fetcher closure.
    fn is_inside_local_resource_fetcher(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let mut closure_depth = 0_u8;
        for (_, node) in cx.tcx.hir_parent_iter(expression.hir_id) {
            let Node::Expr(parent) = node else {
                continue;
            };
            if matches!(parent.kind, ExprKind::Closure(_)) {
                closure_depth = closure_depth.saturating_add(1);
            }

            // The resource constructor closes the ancestry search at its fetcher boundary.
            if Self::is_local_resource_new(cx, parent) {
                return closure_depth == 1;
            }
        }
        false
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosManualResourceRefetchSignals {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Only discarded reactive reads directly inside a fetcher are manual refetch signals.
        if !Self::is_reactive_get(cx, expression)
            || !Self::is_discarded(cx, expression)
            || !Self::is_inside_local_resource_fetcher(cx, expression)
        {
            return;
        }

        Violation {
            owner: expression.hir_id,
            span: expression.span,
        }
        .emit(cx);
    }
}
