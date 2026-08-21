extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

/// Identifies a method through its defining crate and trait contract.
#[derive(Clone, Copy)]
struct TraitMethodIdentity {
    /// Crate that defines the trait.
    defining_crate: &'static str,
    /// Associated method name.
    method: &'static str,
    /// Trait that owns the method.
    owning_trait: &'static str,
}

// -----------------------------------------------------------------------------
// Violation: Manual view iteration diagnostic
// -----------------------------------------------------------------------------

/// Repeated view structure hidden inside an iterator collection chain.
struct Violation {
    /// Collection expression used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored rendering chain highlighted by the diagnostic.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("views are collected through manual iterator mapping")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "iterator-driven rendering hides the collection, identity key, and child template inside Rust expressions instead of exposing them in the view tree",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("replace this mapping chain with `<For>` and a stable domain key")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MANUAL_VIEW_ITERATION,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this repeated view is hidden in an iterator chain",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosManualViewIteration: Declarative collection rendering
// -----------------------------------------------------------------------------

/// Late lint pass that requires repeated views to use declarative Leptos markup.
struct LeptosManualViewIteration;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MANUAL_VIEW_ITERATION,
    Warn,
    "requires declarative Leptos For components instead of mapped view collection",
    LeptosManualViewIteration
}

impl LeptosManualViewIteration {
    /// Returns whether a method belongs to a named semantic trait.
    fn is_method_owned_by(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        identity: TraitMethodIdentity,
    ) -> bool {
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Unresolved calls cannot be attributed to the required semantic trait method.
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        cx.tcx.crate_name(method.krate).as_str() == identity.defining_crate
            && cx.tcx.item_name(method).as_str() == identity.method
            && cx.tcx.trait_of_assoc(method).is_some_and(|trait_id| {
                cx.tcx.item_name(trait_id).as_str() == identity.owning_trait
            })
    }

    /// Returns whether an iterator adapter can transform collection items into child views.
    fn is_view_mapping_adapter(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        ["map", "filter_map", "flat_map", "map_while", "scan"]
            .into_iter()
            .any(|method| {
                Self::is_method_owned_by(
                    cx,
                    expression,
                    TraitMethodIdentity {
                        defining_crate: "core",
                        method,
                        owning_trait: "Iterator",
                    },
                )
            })
    }

    /// Returns whether a receiver chain contains a standard view-mapping adapter.
    fn contains_view_mapping_adapter(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // A resolved mapping adapter establishes manual view construction for the whole chain.
        if Self::is_view_mapping_adapter(cx, expression) {
            return true;
        }

        // A non-method expression terminates the authored iterator chain.
        let ExprKind::MethodCall(_, receiver, _, _) = expression.kind else {
            return false;
        };

        // Receiver traversal preserves semantic method identity across intervening adapters.
        Self::contains_view_mapping_adapter(cx, receiver)
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosManualViewIteration {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Only method calls can terminate in Leptos's collection-view conversion.
        let ExprKind::MethodCall(_, receiver, arguments, _) = expression.kind else {
            return;
        };

        // Calls with arguments or a different semantic method are not `collect_view` terminals.
        if !arguments.is_empty()
            || !Self::is_method_owned_by(
                cx,
                expression,
                TraitMethodIdentity {
                    defining_crate: "leptos",
                    method: "collect_view",
                    owning_trait: "CollectView",
                },
            )
        {
            return;
        }

        // Any standard mapping adapter in the receiver chain hides repeated view structure.
        if !Self::contains_view_mapping_adapter(cx, receiver) {
            return;
        }

        Violation {
            owner: expression.hir_id,
            span: expression.span,
        }
        .emit(cx);
    }
}
