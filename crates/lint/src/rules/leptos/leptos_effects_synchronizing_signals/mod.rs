extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BodyId, Expr, ExprKind, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Effect synchronizing signals through reactive feedback
// -----------------------------------------------------------------------------

/// One effect that reads and writes within the reactive graph.
struct Violation {
    /// Effect call used to honor local lint attributes.
    owner: HirId,
    /// First tracked read that makes the effect reactive.
    read_span: Span,
    /// First reactive write performed by the effect.
    write_span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("effect synchronizes one reactive value into another")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derived state stored by an effect creates a second source of truth, an extra propagation step, and a possible reactive cycle",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive the value with a closure or `Memo`, or update related state together at the originating event",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_EFFECTS_SYNCHRONIZING_SIGNALS,
            self.owner,
            self.write_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.write_span, "this effect writes reactive state");
                diag.span_note(self.read_span, "the same effect tracks reactive state here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Classifies the graph operations performed by one reactive method.
struct ReactiveMethod {
    /// Whether the call establishes a tracked read.
    is_read: bool,
    /// Whether the call writes reactive state.
    is_write: bool,
}

/// Reactive reads and writes directly owned by one effect callback.
struct ReactiveOperations<'analysis, 'tcx> {
    /// Compiler context used for semantic method resolution.
    cx: &'analysis LateContext<'tcx>,
    /// First tracked reactive read.
    read_span: Option<Span>,
    /// First reactive write.
    write_span: Option<Span>,
}

impl<'analysis, 'tcx> ReactiveOperations<'analysis, 'tcx> {
    /// Creates an empty operation summary.
    const fn new(cx: &'analysis LateContext<'tcx>) -> Self {
        Self {
            cx,
            read_span: None,
            write_span: None,
        }
    }

    /// Classifies a method call by its semantic reactive graph trait.
    fn reactive_method(&self, expression: &Expr<'_>) -> Option<ReactiveMethod> {
        let ExprKind::MethodCall(_, _, _, _) = expression.kind else {
            return None;
        };
        let owner = self.cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        let method = self
            .cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)?;

        if self.cx.tcx.crate_name(method.krate).as_str() != "reactive_graph" {
            return None;
        }
        let trait_id = self.cx.tcx.trait_of_assoc(method)?;
        let trait_symbol = self.cx.tcx.item_name(trait_id);
        let method_symbol = self.cx.tcx.item_name(method);
        let trait_name = trait_symbol.as_str();

        let method_name = method_symbol.as_str();

        Some(ReactiveMethod {
            is_read: matches!(
                (trait_name, method_name),
                ("Get", "get") | ("Read", "read") | ("With", "with")
            ),
            is_write: matches!(trait_name, "Set" | "Update" | "UpdateUntracked"),
        })
    }
}

impl<'tcx> Visitor<'tcx> for ReactiveOperations<'_, 'tcx> {
    fn visit_nested_body(&mut self, _: BodyId) {}

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let Some(ReactiveMethod { is_read, is_write }) = self.reactive_method(expression) {
            if self.read_span.is_none() && is_read {
                self.read_span = Some(expression.span);
            }
            if self.write_span.is_none() && is_write {
                self.write_span = Some(expression.span);
            }
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// LeptosEffectsSynchronizingSignals: Acyclic reactive-state policy
// -----------------------------------------------------------------------------

/// Late lint pass that keeps reactive-to-reactive synchronization out of effects.
struct LeptosEffectsSynchronizingSignals;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_EFFECTS_SYNCHRONIZING_SIGNALS,
    Warn,
    "rejects effects that synchronize reactive state into other reactive state",
    LeptosEffectsSynchronizingSignals
}

impl LeptosEffectsSynchronizingSignals {
    /// Returns the framework effect operation selected by an associated call.
    fn effect_operation(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<String> {
        let ExprKind::Call(callee, _) = expression.kind else {
            return None;
        };
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };

        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return None;
        };
        let implementation = cx.tcx.impl_of_assoc(method)?;

        let definition = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()?;

        (cx.tcx.crate_name(method.krate).as_str() == "reactive_graph"
            && cx.tcx.item_name(definition.did()).as_str() == "Effect")
            .then(|| cx.tcx.item_name(method).as_str().to_owned())
    }

    /// Visits one direct closure argument and returns its reactive operations.
    fn closure_operations<'analysis, 'tcx>(
        cx: &'analysis LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> Option<ReactiveOperations<'analysis, 'tcx>> {
        let ExprKind::Closure(closure) = expression.kind else {
            return None;
        };
        let mut operations = ReactiveOperations::new(cx);
        operations.visit_body(cx.tcx.hir_body(closure.body));
        Some(operations)
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosEffectsSynchronizingSignals {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        /// Dependency and callback closures supplied to `watch`.
        // Bound the operation-specific closure scan.
        const WATCH_CLOSURE_COUNT: usize = 2;

        let ExprKind::Call(_, arguments) = expression.kind else {
            return;
        };
        let Some(operation) = Self::effect_operation(cx, expression) else {
            return;
        };

        let mut read_span = None;
        let mut write_span = None;
        let closure_count = match operation.as_str() {
            "new" => 1,
            "watch" => WATCH_CLOSURE_COUNT,
            _ => return,
        };

        for argument in arguments.iter().take(closure_count) {
            let Some(operations) = Self::closure_operations(cx, argument) else {
                continue;
            };
            read_span = read_span.or(operations.read_span);
            write_span = write_span.or(operations.write_span);
        }

        let (Some(read_span), Some(write_span)) = (read_span, write_span) else {
            return;
        };

        Violation {
            owner: expression.hir_id,
            read_span,
            write_span,
        }
        .emit(cx);
    }
}
