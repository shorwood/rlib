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
// Violation: Unkeyed reactive collection diagnostic
// -----------------------------------------------------------------------------

/// Reactive collection rendered through positional iterator collection.
struct Violation {
    /// Collection expression used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored rendering chain highlighted by the diagnostic.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("reactive collection is rendered without stable keys")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "collecting mapped views identifies children only by position, so filtering, reordering, or insertion can recreate unrelated DOM state",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("render the changing collection with `<For>` and a stable domain key")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_UNKEYED_REACTIVE_COLLECTIONS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "these children are collected positionally");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosUnkeyedReactiveCollections: Child identity policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires stable identity for changing reactive collections.
struct LeptosUnkeyedReactiveCollections;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNKEYED_REACTIVE_COLLECTIONS,
    Warn,
    "rejects positionally collected views derived from reactive collections",
    LeptosUnkeyedReactiveCollections
}

impl LeptosUnkeyedReactiveCollections {
    /// Returns whether a method belongs to a named semantic trait.
    fn method_belongs_to(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        identity: TraitMethodIdentity,
    ) -> bool {
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
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

    /// Returns whether the rendering chain originates in a tracked signal clone.
    fn chain_contains_reactive_get(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let ExprKind::MethodCall(_, receiver, arguments, _) = expression.kind else {
            return false;
        };
        if arguments.is_empty()
            && Self::method_belongs_to(
                cx,
                expression,
                TraitMethodIdentity {
                    defining_crate: "reactive_graph",
                    method: "get",
                    owning_trait: "Get",
                },
            )
        {
            return true;
        }
        Self::chain_contains_reactive_get(cx, receiver)
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnkeyedReactiveCollections {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(_, receiver, arguments, _) = expression.kind else {
            return;
        };
        if !arguments.is_empty()
            || !Self::method_belongs_to(
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
        let ExprKind::MethodCall(map, collection, [_], _) = receiver.kind else {
            return;
        };

        if map.ident.name.as_str() != "map" || !Self::chain_contains_reactive_get(cx, collection) {
            return;
        }

        Violation {
            owner: expression.hir_id,
            span: expression.span,
        }
        .emit(cx);
    }
}
