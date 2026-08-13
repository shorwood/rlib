extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, ImplItem, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::contracts::StrumAssociatedItem;
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("enum declaration order is used as a domain ordering contract")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "reordering variants changes workflow or presentation behavior without changing an explicit policy",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("introduce a named ordering method or constant, or sort by an explicit key")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_DECLARATION_ORDER_DOMAIN_CONTRACTS,
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

/// Performs the `order_sensitive_context` step of the lint analysis.
fn order_sensitive_context(cx: &LateContext<'_>, hir_id: rustc_hir::HirId) -> bool {
    let name = cx
        .tcx
        .hir_parent_iter(hir_id)
        .find_map(|(_, node)| match node {
            Node::Item(item) => item.kind.ident().map(|ident| ident.name),
            Node::ImplItem(ImplItem { ident, .. }) => Some(ident.name),
            _ => None,
        });
    name.is_some_and(|name| {
        [
            "workflow",
            "migration",
            "phase",
            "priority",
            "protocol",
            "menu",
            "present",
            "render",
            "execute",
        ]
        .into_iter()
        .any(|token| name.as_str().contains(token))
    })
}

/// Carries the `StrumDeclarationOrderDomainContracts` state used by this analysis.
struct StrumDeclarationOrderDomainContracts;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_DECLARATION_ORDER_DOMAIN_CONTRACTS,
    Warn,
    "finds declaration-order Strum collections used as domain order",
    StrumDeclarationOrderDomainContracts
}

impl LateLintPass<'_> for StrumDeclarationOrderDomainContracts {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Reject inputs that do not satisfy this stage.
        if expression.span.from_expansion() || !order_sensitive_context(cx, expression.hir_id) {
            return;
        }

        // Prepare the values used by this stage.
        let associated = match expression.kind {
            ExprKind::Call(callee, []) => Some(callee),
            ExprKind::Path(_)
                if !matches!(
                    cx.tcx.parent_hir_node(expression.hir_id),
                    Node::Expr(Expr {
                        kind: ExprKind::Call(_, []),
                        ..
                    })
                ) =>
            {
                Some(expression)
            }
            _ => None,
        };

        // Prepare the values used by this stage.
        let Some(associated) = associated else { return };
        let is_order_source = (StrumAssociatedItem {
            trait_name: "IntoEnumIterator",
            item_name: "iter",
        })
        .enum_definition(cx, associated)
        .is_some()
            || (StrumAssociatedItem {
                trait_name: "VariantArray",
                item_name: "VARIANTS",
            })
            .enum_definition(cx, associated)
            .is_some();

        // Reject inputs that do not satisfy this stage.
        if !(is_order_source) {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span.source_callsite(),
        }
        .emit(cx);
    }
}
