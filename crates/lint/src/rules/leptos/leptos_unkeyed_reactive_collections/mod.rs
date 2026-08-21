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

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNKEYED_REACTIVE_COLLECTIONS,
    Warn,
    "rejects positionally collected views derived from reactive collections",
    LeptosUnkeyedReactiveCollections
}

impl LeptosUnkeyedReactiveCollections {
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

    /// Returns whether the rendering chain originates in a tracked signal read.
    fn contains_reactive_read(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // A non-method expression terminates the receiver chain without a tracked read.
        let ExprKind::MethodCall(_, receiver, arguments, _) = expression.kind else {
            return false;
        };
        let tracked_read = [
            ("Get", "get", 0),
            ("Get", "try_get", 0),
            ("Read", "read", 0),
            ("Read", "try_read", 0),
            ("With", "with", 1),
            ("With", "try_with", 1),
        ]
        .into_iter()
        .any(|(owning_trait, method, argument_count)| {
            arguments.len() == argument_count
                && Self::is_method_owned_by(
                    cx,
                    expression,
                    TraitMethodIdentity {
                        defining_crate: "reactive_graph",
                        method,
                        owning_trait,
                    },
                )
        });

        // Finding a tracked read anywhere in the chain establishes reactive collection input.
        if tracked_read {
            return true;
        }
        Self::contains_reactive_read(cx, receiver)
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnkeyedReactiveCollections {
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

        // A direct mapping adapter must immediately precede collection into a view.
        let ExprKind::MethodCall(_, collection, [_], _) = receiver.kind else {
            return;
        };

        // Only mapped children sourced from a tracked read create the unkeyed reactive pattern.
        if !Self::is_view_mapping_adapter(cx, receiver)
            || !Self::contains_reactive_read(cx, collection)
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
