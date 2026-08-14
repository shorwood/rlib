extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BodyId, Expr, ExprKind, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// ReactiveMethodIdentity: Type and behavior
// -----------------------------------------------------------------------------

/// Identifies one method through its reactive trait contract.
#[derive(Clone, Copy)]
struct ReactiveMethodIdentity {
    /// Reactive graph trait that owns the method.
    trait_name: &'static str,
    /// Associated method name.
    method_name: &'static str,
}

/// Authored place that identifies one writable signal without relying on its concrete wrapper.
#[derive(Clone, PartialEq, Eq)]
struct SignalPlace {
    /// Local binding at the root of the place.
    root: HirId,
    /// Named field projections below the root.
    fields: Vec<Symbol>,
}

// -----------------------------------------------------------------------------
// Violation: Read then replace diagnostic
// -----------------------------------------------------------------------------

/// Replacement whose value depends on the same signal's current value.
struct Violation {
    /// Replacement expression used to honor local lint attributes.
    owner: HirId,
    /// Authored replacement highlighted by the diagnostic.
    span: Span,
    /// Current-value read that makes the replacement stale-prone.
    read_span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("signal is read and then replaced")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a separate read can clone the stored value and lets the replacement depend on an older snapshot",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "perform the change with `update(...)` so the mutation stays within one write",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_READ_THEN_REPLACE_SIGNALS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this replacement depends on the current value");
                diag.span_label(self.read_span, "the same signal is read here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// CurrentValueRead: Replacement value analysis
// -----------------------------------------------------------------------------

/// Finds a tracked current-value read from one local signal binding.
struct CurrentValueRead<'analysis, 'tcx> {
    /// Compiler context used for semantic method resolution.
    cx: &'analysis LateContext<'tcx>,
    /// Body owner whose type-checking results resolve method calls.
    owner: LocalDefId,
    /// Signal place being replaced.
    signal: SignalPlace,
    /// First matching read span.
    span: Option<Span>,
}

impl<'tcx> Visitor<'tcx> for CurrentValueRead<'_, 'tcx> {
    fn visit_nested_body(&mut self, _: BodyId) {}

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.span.is_some() {
            return;
        }

        // An immediately invoked closure contributes to the replacement value. Other nested
        // bodies can be retained callbacks and are deliberately not treated as eager reads.
        if let ExprKind::Call(callee, arguments) = expression.kind
            && let ExprKind::Closure(closure) = callee.kind
        {
            self.visit_body(self.cx.tcx.hir_body(closure.body));
            for argument in arguments {
                self.visit_expr(argument);
            }
            return;
        }

        let ExprKind::MethodCall(_, receiver, arguments, _) = expression.kind else {
            intravisit::walk_expr(self, expression);
            return;
        };

        if LeptosReadThenReplaceSignals::signal_place(self.cx, receiver)
            .is_some_and(|place| place == self.signal)
            && LeptosReadThenReplaceSignals::is_current_value_read(
                self.cx,
                self.owner,
                expression,
                arguments.len(),
            )
        {
            self.span = Some(expression.span);
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// LeptosReadThenReplaceSignals: Atomic update policy
// -----------------------------------------------------------------------------

/// Late lint pass that keeps dependent signal changes inside `update`.
struct LeptosReadThenReplaceSignals;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_READ_THEN_REPLACE_SIGNALS,
    Warn,
    "rejects signal replacements calculated from the same signal's current value",
    LeptosReadThenReplaceSignals
}

impl LeptosReadThenReplaceSignals {
    /// Resolves a local path and its named field projections as a stable signal place.
    fn signal_place(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<SignalPlace> {
        match expression.kind {
            ExprKind::Path(path) => {
                let Res::Local(root) = cx.qpath_res(&path, expression.hir_id) else {
                    return None;
                };
                Some(SignalPlace {
                    root,
                    fields: Vec::new(),
                })
            }
            ExprKind::Field(base, field) => {
                let mut place = Self::signal_place(cx, base)?;
                place.fields.push(field.name);
                Some(place)
            }
            ExprKind::AddrOf(_, _, inner) => Self::signal_place(cx, inner),
            _ => None,
        }
    }

    /// Returns whether a call resolves to one reactive graph trait method.
    fn is_reactive_method(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &Expr<'_>,
        identity: ReactiveMethodIdentity,
    ) -> bool {
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        cx.tcx.crate_name(method.krate).as_str() == "reactive_graph"
            && cx.tcx.item_name(method).as_str() == identity.method_name
            && cx
                .tcx
                .trait_of_assoc(method)
                .is_some_and(|trait_id| cx.tcx.item_name(trait_id).as_str() == identity.trait_name)
    }

    /// Returns whether a method reads the receiver's current reactive value.
    fn is_current_value_read(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &Expr<'_>,
        argument_count: usize,
    ) -> bool {
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };
        if cx.tcx.crate_name(method.krate).as_str() != "reactive_graph" {
            return false;
        }
        let Some(trait_id) = cx.tcx.trait_of_assoc(method) else {
            return false;
        };

        matches!(
            (
                cx.tcx.item_name(trait_id).as_str(),
                cx.tcx.item_name(method).as_str(),
                argument_count,
            ),
            ("Get", "get" | "try_get", 0)
                | ("GetUntracked", "get_untracked" | "try_get_untracked", 0)
                | ("Read", "read" | "try_read", 0)
                | ("ReadUntracked", "read_untracked" | "try_read_untracked", 0)
                | ("With", "with" | "try_with", 1)
                | ("WithUntracked", "with_untracked" | "try_with_untracked", 1)
        )
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosReadThenReplaceSignals {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(_, receiver, [replacement], _) = expression.kind else {
            return;
        };
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let Some(signal) = Self::signal_place(cx, receiver) else {
            return;
        };

        let is_replacement = ["set", "try_set"].into_iter().any(|method_name| {
            Self::is_reactive_method(
                cx,
                owner,
                expression,
                ReactiveMethodIdentity {
                    trait_name: "Set",
                    method_name,
                },
            )
        });
        if !is_replacement {
            return;
        }

        let mut read = CurrentValueRead {
            cx,
            owner,
            signal,
            span: None,
        };
        read.visit_expr(replacement);

        let Some(read_span) = read.span else {
            return;
        };

        Violation {
            owner: expression.hir_id,
            span: expression.span,
            read_span,
        }
        .emit(cx);
    }
}
