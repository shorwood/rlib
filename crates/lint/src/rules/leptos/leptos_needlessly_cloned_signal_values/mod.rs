extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, TypingEnv};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Needlessly cloned signal value diagnostic
// -----------------------------------------------------------------------------

/// Cloning reactive read whose value is only inspected through a shared borrow.
struct Violation {
    /// Borrowing consumer used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored cloning read highlighted by the diagnostic.
    span: Span,
    /// Borrowing operation that consumes the cloned value.
    operation: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "signal value is cloned only to call `{}`",
            self.operation
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "tracked `get()` clones the complete stored value, while this operation only needs a temporary shared borrow",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("borrow the signal value with `read()` or a bounded `with(...)` closure")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_NEEDLESSLY_CLONED_SIGNAL_VALUES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this tracked read clones the stored value");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosNeedlesslyClonedSignalValues: Reactive read policy
// -----------------------------------------------------------------------------

/// Late lint pass that borrows non-copy signal values for immediate inspection.
struct LeptosNeedlesslyClonedSignalValues;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_NEEDLESSLY_CLONED_SIGNAL_VALUES,
    Warn,
    "rejects cloned signal values used only by an immediate borrowing operation",
    LeptosNeedlesslyClonedSignalValues
}

impl LeptosNeedlesslyClonedSignalValues {
    /// Returns whether the receiver resolves to tracked reactive `get()`.
    fn is_reactive_get(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // Only method calls can implement the tracked reactive read interface.
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return false;
        };

        // Tracked get operations accept no explicit arguments.
        if !arguments.is_empty() {
            return false;
        }
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Unresolved calls cannot be classified through their reactive trait contract.
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

    /// Returns whether the cloned result type has inexpensive copy semantics.
    fn is_copy(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let ty = cx.tcx.typeck(owner).expr_ty(expression);
        cx.tcx
            .type_is_copy_modulo_regions(TypingEnv::post_analysis(cx.tcx, owner), ty)
    }

    /// Recognizes the deliberately narrow family of inspection method names.
    fn borrowing_operation_name(name: &str, argument_count: usize) -> bool {
        matches!((name, argument_count), ("len" | "is_empty", 0))
    }

    /// Accepts only resolved operations whose receiver is shared-borrowed for the call.
    fn is_borrowing_operation(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // Borrow-only consumers must be expressed as receiver method calls.
        let ExprKind::MethodCall(segment, _, arguments, _) = expression.kind else {
            return false;
        };

        // Exclude methods outside the deliberately narrow inspection vocabulary.
        if !Self::borrowing_operation_name(segment.ident.name.as_str(), arguments.len()) {
            return false;
        }
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Unresolved methods cannot prove a shared-borrow receiver contract.
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };
        let signature = cx.tcx.fn_sig(method).instantiate_identity().skip_binder();
        signature.inputs().first().is_some_and(|receiver| {
            matches!(receiver.kind(), ty::Ref(_, _, rustc_hir::Mutability::Not))
        })
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosNeedlesslyClonedSignalValues {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // The borrowing consumer must be expressed as a receiver method call.
        let ExprKind::MethodCall(segment, receiver, _, _) = expression.kind else {
            return;
        };

        // Emit only for noncopy tracked clones consumed immediately through a shared borrow.
        if !Self::is_borrowing_operation(cx, expression)
            || !Self::is_reactive_get(cx, receiver)
            || Self::is_copy(cx, receiver)
        {
            return;
        }

        Violation {
            owner: expression.hir_id,
            span: receiver.span,
            operation: segment.ident.name,
        }
        .emit(cx);
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::LeptosNeedlesslyClonedSignalValues;

    #[test]
    fn recognizes_borrow_only_operations() {
        assert!(LeptosNeedlesslyClonedSignalValues::borrowing_operation_name("len", 0));
        assert!(LeptosNeedlesslyClonedSignalValues::borrowing_operation_name("is_empty", 0));
        assert!(!LeptosNeedlesslyClonedSignalValues::borrowing_operation_name("into_iter", 0));
    }
}
