extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Reactive render write diagnostic
// -----------------------------------------------------------------------------

/// Reactive mutation performed directly while an asynchronous view is being resolved.
struct Violation {
    /// Write expression used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored mutation highlighted by the diagnostic.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("reactive state is written while constructing a view")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "view construction may run again when asynchronous or reactive inputs change, so this write can reset newer user state and trigger another render",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "initialize the state during component setup, or move the loaded data into a child component whose setup owns that state",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_REACTIVE_WRITES_DURING_VIEW_CONSTRUCTION,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this write runs as the suspended view resolves");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosReactiveWritesDuringViewConstruction: Render purity policy
// -----------------------------------------------------------------------------

/// Late lint pass that keeps direct reactive mutation out of suspended view construction.
struct LeptosReactiveWritesDuringViewConstruction;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_REACTIVE_WRITES_DURING_VIEW_CONSTRUCTION,
    Warn,
    "rejects reactive writes performed while constructing a Leptos view",
    LeptosReactiveWritesDuringViewConstruction
}

impl LeptosReactiveWritesDuringViewConstruction {
    /// Returns whether this method is a mutation operation from the reactive graph.
    fn is_reactive_write(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // Resolve the authored method call to its defining trait.
        let ExprKind::MethodCall(_, _, _, _) = expression.kind else {
            return false;
        };

        // Look up the method selected by type checking this body.
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        // Discard identically named methods outside the reactive graph crate.
        if cx.tcx.crate_name(method.krate).as_str() != "reactive_graph" {
            return false;
        }

        // Restrict mutation evidence to the reactive graph's write traits.
        cx.tcx.trait_of_assoc(method).is_some_and(|trait_id| {
            matches!(
                cx.tcx.item_name(trait_id).as_str(),
                "Set" | "Update" | "UpdateUntracked"
            )
        })
    }

    /// Returns whether the call resolves to `tachys::Suspend::new`.
    fn is_suspend_new(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // Resolve the called associated function and its implementing type.
        let ExprKind::Call(callee, _) = expression.kind else {
            return false;
        };

        // Require a direct path to an associated function.
        let ExprKind::Path(path) = callee.kind else {
            return false;
        };

        // Resolve that path to its function definition.
        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return false;
        };

        // Recover the implementation and its concrete self type.
        let Some(implementation) = cx.tcx.impl_of_assoc(method) else {
            return false;
        };
        let Some(definition) = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()
        else {
            return false;
        };

        // Match only the framework's suspend constructor.
        cx.tcx.crate_name(method.krate).as_str() == "tachys"
            && cx.tcx.item_name(method).as_str() == "new"
            && cx.tcx.item_name(definition.did()).as_str() == "Suspend"
    }

    /// Proves the write belongs directly to a suspended future rather than a nested callback.
    fn is_direct_suspend_write(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let mut closure_depth = 0_u8;
        for (_, node) in cx.tcx.hir_parent_iter(expression.hir_id) {
            let Node::Expr(parent) = node else {
                continue;
            };
            if matches!(parent.kind, ExprKind::Closure(_)) {
                closure_depth = closure_depth.saturating_add(1);
            }
            if Self::is_suspend_new(cx, parent) {
                return closure_depth == 1;
            }
        }
        false
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosReactiveWritesDuringViewConstruction {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        if !Self::is_reactive_write(cx, expression)
            || !Self::is_direct_suspend_write(cx, expression)
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
