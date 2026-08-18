extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::hygiene::{ExpnKind, MacroKind};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Frozen view value diagnostic
// -----------------------------------------------------------------------------

/// Tracked value read eagerly while a view is constructed.
struct Violation {
    /// Read expression used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored eager read highlighted by the diagnostic.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("tracked signal value is frozen into the initial view")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "component setup runs once, so an eagerly read value is no longer reactive when the source changes",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("pass the signal directly, or wrap the derived read in a `move ||` closure")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_UNREACTIVE_SIGNAL_READS_IN_VIEWS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this read happens only during view construction");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosUnreactiveSignalReadsInViews: View reactivity policy
// -----------------------------------------------------------------------------

/// Late lint pass that keeps tracked view reads behind reactive closures.
struct LeptosUnreactiveSignalReadsInViews;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNREACTIVE_SIGNAL_READS_IN_VIEWS,
    Warn,
    "rejects tracked signal reads frozen into initial Leptos views",
    LeptosUnreactiveSignalReadsInViews
}

impl LeptosUnreactiveSignalReadsInViews {
    /// Returns whether the expression resolves to a tracked reactive read.
    fn is_tracked_read(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // Non-method expressions cannot resolve to a tracked read trait method.
        let ExprKind::MethodCall(_, _, _, _) = expression.kind else {
            return false;
        };
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Unresolved method calls cannot establish reactive read semantics.
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        // Methods outside the reactive graph crate are not Leptos tracked reads.
        if cx.tcx.crate_name(method.krate).as_str() != "reactive_graph" {
            return false;
        }

        // Inherent methods cannot implement one of the tracked read trait contracts.
        let Some(trait_id) = cx.tcx.trait_of_assoc(method) else {
            return false;
        };

        matches!(
            (
                cx.tcx.item_name(trait_id).as_str(),
                cx.tcx.item_name(method).as_str()
            ),
            ("Get", "get" | "try_get")
                | ("Read", "read" | "try_read")
                | ("With", "with" | "try_with")
        )
    }

    /// Returns whether a span originates in authored `view!` input.
    fn is_view_expansion(span: Span) -> bool {
        span.macro_backtrace().any(|expansion| {
            matches!(expansion.kind, ExpnKind::Macro(MacroKind::Bang, name) if name.as_str() == "view")
        })
    }

    /// Returns whether the read belongs to a view without an authored closure boundary.
    fn is_eager_view_read(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let mut is_inside_view = Self::is_view_expansion(expression.span);
        for (_, node) in cx.tcx.hir_parent_iter(expression.hir_id) {
            let Node::Expr(parent) = node else {
                continue;
            };
            is_inside_view |= Self::is_view_expansion(parent.span);

            // An authored closure restores reactive reevaluation for reads beneath it.
            if matches!(parent.kind, ExprKind::Closure(_))
                && cx
                    .sess()
                    .source_map()
                    .span_to_snippet(parent.span)
                    .is_ok_and(|source| syn::parse_str::<syn::ExprClosure>(&source).is_ok())
            {
                return false;
            }
        }
        is_inside_view
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnreactiveSignalReadsInViews {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Only tracked reads evaluated eagerly inside authored views violate the policy.
        if !Self::is_tracked_read(cx, expression) || !Self::is_eager_view_read(cx, expression) {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span,
        }
        .emit(cx);
    }
}
