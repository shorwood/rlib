extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BodyId, Expr, ExprKind, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Resource fetcher rereading its reactive source
// -----------------------------------------------------------------------------

/// Resource fetcher that rereads a signal already represented by its source value.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("resource fetcher rereads its tracked source")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "reactive reads in the source closure are tracked, while fetcher reads are not and may observe a different value",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("name the fetcher argument and use it as the resource's complete input")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_RESOURCE_FETCHERS_REREADING_SOURCES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this read bypasses the tracked source value");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Selects the reactive read families relevant to one resource closure.
#[derive(Clone, Copy)]
enum ReactiveReadCollection {
    /// Only tracked reads establish resource source dependencies.
    TrackedSource,
    /// Any current-value read in the fetcher can bypass its supplied input.
    Fetcher,
}

/// Local signal place, including named field projections.
#[derive(Clone, PartialEq, Eq)]
struct SignalPlace {
    /// Root local binding.
    root: HirId,
    /// Field path below the root.
    fields: Vec<Symbol>,
}

/// One reactive binding read and its authored expression span.
struct ReactiveRead {
    /// Signal place read by the expression.
    place: SignalPlace,
    /// Authored read expression span.
    span: Span,
}

/// Reactive source reads observed while visiting one resource fetcher.
struct ReactiveReads<'analysis, 'tcx> {
    /// Compiler context used to resolve calls and captured bindings.
    cx: &'analysis LateContext<'tcx>,
    /// Reactive getters called from the fetcher closure.
    reads: Vec<ReactiveRead>,
    /// Read families accepted for this closure.
    collection: ReactiveReadCollection,
    /// Nesting depth used to avoid attributing reads from nested closures.
    body_depth: u8,
}

impl<'analysis, 'tcx> ReactiveReads<'analysis, 'tcx> {
    /// Deepest closure body that executes as part of the analyzed resource closure.
    const MAXIMUM_RESOURCE_BODY_DEPTH: u8 = 2;

    /// Starts reactive-read collection for one resource fetcher body.
    const fn new(cx: &'analysis LateContext<'tcx>, collection: ReactiveReadCollection) -> Self {
        Self {
            cx,
            reads: Vec::new(),
            collection,
            body_depth: 0,
        }
    }

    /// Recognizes a reactive current-value read relevant to this closure.
    fn is_reactive_read(&self, expression: &Expr<'_>) -> bool {
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return false;
        };
        let owner = self.cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        let Some(method) = self
            .cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        if self.cx.tcx.crate_name(method.krate).as_str() != "reactive_graph" {
            return false;
        }
        let Some(trait_id) = self.cx.tcx.trait_of_assoc(method) else {
            return false;
        };
        let trait_name = self.cx.tcx.item_name(trait_id);
        let method_name = self.cx.tcx.item_name(method);
        let identity = (trait_name.as_str(), method_name.as_str(), arguments.len());
        let tracked = matches!(
            identity,
            ("Get", "get" | "try_get", 0)
                | ("Read", "read" | "try_read", 0)
                | ("With", "with" | "try_with", 1)
        );
        tracked
            || matches!(self.collection, ReactiveReadCollection::Fetcher)
                && matches!(
                    identity,
                    ("GetUntracked", "get_untracked" | "try_get_untracked", 0)
                        | ("ReadUntracked", "read_untracked" | "try_read_untracked", 0)
                        | ("WithUntracked", "with_untracked" | "try_with_untracked", 1)
                )
    }
}

impl<'tcx> Visitor<'tcx> for ReactiveReads<'_, 'tcx> {
    fn visit_nested_body(&mut self, body_id: BodyId) {
        if self.body_depth >= Self::MAXIMUM_RESOURCE_BODY_DEPTH {
            return;
        }
        self.body_depth += 1;
        self.visit_body(self.cx.tcx.hir_body(body_id));
        self.body_depth -= 1;
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.is_reactive_read(expression)
            && let ExprKind::MethodCall(_, receiver, _, _) = expression.kind
            && let Some(place) =
                LeptosResourceFetchersRereadingSources::signal_place(self.cx, receiver)
            && !self.reads.iter().any(|read| read.place == place)
        {
            self.reads.push(ReactiveRead {
                place,
                span: expression.span,
            });
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Source and fetcher closures passed to a resource constructor.
struct ResourceClosures<'tcx> {
    /// Tracked source closure.
    source: &'tcx Expr<'tcx>,
    /// Asynchronous fetcher closure.
    fetcher: &'tcx Expr<'tcx>,
}

// -----------------------------------------------------------------------------
// LeptosResourceFetchersRereadingSources: Explicit fetcher-input policy
// -----------------------------------------------------------------------------

/// Finds fetchers that reread signals instead of using their tracked source value.
struct LeptosResourceFetchersRereadingSources;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_RESOURCE_FETCHERS_REREADING_SOURCES,
    Warn,
    "rejects Leptos resource fetchers that reread their tracked sources",
    LeptosResourceFetchersRereadingSources
}

impl LeptosResourceFetchersRereadingSources {
    /// Extracts the source and fetcher closures from a resource constructor call.
    fn closures<'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> Option<ResourceClosures<'tcx>> {
        let ExprKind::Call(callee, arguments) = expression.kind else {
            return None;
        };
        let [source, fetcher, ..] = arguments else {
            return None;
        };

        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return None;
        };

        if cx.tcx.crate_name(method.krate).as_str() != "leptos_server"
            || !cx.tcx.item_name(method).as_str().starts_with("new")
        {
            return None;
        }
        let implementation = cx.tcx.impl_of_assoc(method)?;

        let definition = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()?;

        matches!(
            cx.tcx.item_name(definition.did()).as_str(),
            "Resource" | "ArcResource"
        )
        .then_some(ResourceClosures { source, fetcher })
    }

    /// Resolves a local path and its named field projections.
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

    /// Compares tracked source bindings with reactive reads in the fetcher.
    fn analyze<'analysis, 'tcx>(
        cx: &'analysis LateContext<'tcx>,
        closure: &'tcx Expr<'tcx>,
        collection: ReactiveReadCollection,
    ) -> Option<ReactiveReads<'analysis, 'tcx>> {
        let ExprKind::Closure(closure) = closure.kind else {
            return None;
        };
        let body = cx.tcx.hir_body(closure.body);
        let mut analysis = ReactiveReads::new(cx, collection);
        analysis.visit_expr(body.value);
        Some(analysis)
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosResourceFetchersRereadingSources {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(ResourceClosures { source, fetcher }) = Self::closures(cx, expression) else {
            return;
        };
        let Some(source) = Self::analyze(cx, source, ReactiveReadCollection::TrackedSource) else {
            return;
        };

        let Some(fetcher) = Self::analyze(cx, fetcher, ReactiveReadCollection::Fetcher) else {
            return;
        };

        let Some(read) = fetcher.reads.iter().find(|read| {
            source
                .reads
                .iter()
                .any(|source_read| source_read.place == read.place)
        }) else {
            return;
        };

        Violation {
            owner: expression.hir_id,
            span: read.span,
        }
        .emit(cx);
    }
}
