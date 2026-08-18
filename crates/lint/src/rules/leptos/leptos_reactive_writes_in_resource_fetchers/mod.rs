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
// Violation: Resource fetcher mutation diagnostic
// -----------------------------------------------------------------------------

/// Reactive mutation performed while a resource value is loaded.
struct Violation {
    /// Resource construction used to honor local lint attributes.
    owner: HirId,
    /// First reactive mutation in the fetcher.
    write_span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("resource fetcher writes unrelated reactive state")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "resource fetchers do not necessarily execute in every server-rendering, hydration, and client-rendering stage, so this mutation has stage-dependent behavior",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "return the loaded state as part of the resource value, or move event-driven mutation to the event that initiated the load",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_REACTIVE_WRITES_IN_RESOURCE_FETCHERS,
            self.owner,
            self.write_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.write_span,
                    "this write depends on the fetcher executing",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// FetcherWrites: Nested future analysis
// -----------------------------------------------------------------------------

/// Finds the first semantic reactive write inside a resource fetcher.
struct FetcherWrites<'analysis, 'tcx> {
    /// Compiler context used for semantic method resolution.
    cx: &'analysis LateContext<'tcx>,
    /// First reactive write found beneath the fetcher.
    write_span: Option<Span>,
    /// Number of closure bodies between the constructor and current expression.
    body_depth: u8,
}

impl<'analysis, 'tcx> FetcherWrites<'analysis, 'tcx> {
    /// Deepest closure body that still executes as part of fetching a resource.
    const MAXIMUM_FETCHER_BODY_DEPTH: u8 = 2;

    /// Creates an empty fetcher analysis.
    const fn new(cx: &'analysis LateContext<'tcx>) -> Self {
        Self {
            cx,
            write_span: None,
            body_depth: 0,
        }
    }

    /// Returns whether a method is a mutation from a reactive graph write trait.
    fn is_reactive_write(&self, expression: &Expr<'_>) -> bool {
        // Only method calls can implement a reactive write interface.
        let ExprKind::MethodCall(_, _, _, _) = expression.kind else {
            return false;
        };
        let owner = self.cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Unresolved calls cannot be classified through their reactive trait contract.
        let Some(method) = self
            .cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        self.cx.tcx.crate_name(method.krate).as_str() == "reactive_graph"
            && self.cx.tcx.trait_of_assoc(method).is_some_and(|trait_id| {
                matches!(
                    self.cx.tcx.item_name(trait_id).as_str(),
                    "Set" | "Update" | "UpdateUntracked"
                )
            })
    }
}

impl<'tcx> Visitor<'tcx> for FetcherWrites<'_, 'tcx> {
    fn visit_nested_body(&mut self, body_id: BodyId) {
        // Follow the fetcher closure and its returned async body. A deeper closure is a callback
        // merely created by the fetcher and does not execute as part of loading the value.
        // Stop before attributing retained callback mutations to the fetch operation.
        if self.body_depth >= Self::MAXIMUM_FETCHER_BODY_DEPTH {
            return;
        }
        self.body_depth += 1;
        self.visit_body(self.cx.tcx.hir_body(body_id));
        self.body_depth -= 1;
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // The first reactive write is sufficient evidence and the preferred diagnostic anchor.
        if self.write_span.is_none() && self.is_reactive_write(expression) {
            self.write_span = Some(expression.span);
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// LeptosReactiveWritesInResourceFetchers: Fetcher purity policy
// -----------------------------------------------------------------------------

/// Late lint pass that keeps secondary reactive mutation out of resource fetchers.
struct LeptosReactiveWritesInResourceFetchers;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_REACTIVE_WRITES_IN_RESOURCE_FETCHERS,
    Warn,
    "rejects reactive writes performed inside Leptos resource fetchers",
    LeptosReactiveWritesInResourceFetchers
}

impl LeptosReactiveWritesInResourceFetchers {
    /// Returns the fetcher argument of a semantic resource constructor.
    fn fetcher<'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> Option<&'tcx Expr<'tcx>> {
        // Resource construction must be expressed as a direct call.
        let ExprKind::Call(callee, arguments) = expression.kind else {
            return None;
        };

        // Indirect callees cannot identify a resource constructor.
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };

        // Only resolved definitions can be matched to framework constructors.
        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return None;
        };

        // Calls outside the Leptos server crate do not construct these resources.
        if cx.tcx.crate_name(method.krate).as_str() != "leptos_server" {
            return None;
        }
        let constructor = cx.tcx.item_name(method);
        let implementation = cx.tcx.impl_of_assoc(method)?;

        let definition = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()?;

        match (
            cx.tcx.item_name(definition.did()).as_str(),
            constructor.as_str(),
        ) {
            ("Resource" | "ArcResource", name) if name.starts_with("new") => arguments.get(1),
            ("LocalResource" | "ArcLocalResource", "new") => arguments.first(),
            _ => None,
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosReactiveWritesInResourceFetchers {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Unrecognized calls do not establish a resource fetcher boundary.
        let Some(fetcher) = Self::fetcher(cx, expression) else {
            return;
        };
        let mut writes = FetcherWrites::new(cx);
        writes.visit_expr(fetcher);

        // Pure fetchers contain no reactive mutation to report.
        let Some(write_span) = writes.write_span else {
            return;
        };

        Violation {
            owner: expression.hir_id,
            write_span,
        }
        .emit(cx);
    }
}
