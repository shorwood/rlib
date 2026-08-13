extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BodyId, Expr, ExprKind, HirId, Pat, PatKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: HirId,
    /// Stores the `span` value used by this analysis.
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

/// Selects whether closure parameters participate in reactive-read analysis.
#[derive(Clone, Copy)]
enum ReactiveParameterCollection {
    /// Ignore closure parameters when analyzing the source closure.
    Ignore,
    /// Collect closure parameters when analyzing the fetcher closure.
    Collect,
}

/// One reactive binding read and its authored expression span.
struct ReactiveRead {
    /// Binding read by the expression.
    binding: HirId,
    /// Authored read expression span.
    span: Span,
}

/// Carries the `ReactiveReads` state used by this analysis.
struct ReactiveReads<'analysis, 'tcx> {
    /// Stores the `cx` value used by this analysis.
    cx: &'analysis LateContext<'tcx>,
    /// Stores the `reads` value used by this analysis.
    reads: Vec<ReactiveRead>,
    /// Stores the `parameter_bindings` value used by this analysis.
    parameter_bindings: Vec<HirId>,
    /// Stores the `used_parameters` value used by this analysis.
    used_parameters: Vec<HirId>,
    /// Stores the `body_depth` value used by this analysis.
    body_depth: u8,
}

impl<'analysis, 'tcx> ReactiveReads<'analysis, 'tcx> {
    /// Deepest closure body that executes as part of the analyzed resource closure.
    const MAXIMUM_RESOURCE_BODY_DEPTH: u8 = 2;

    /// Performs the `new` operation for this value.
    const fn new(cx: &'analysis LateContext<'tcx>) -> Self {
        Self {
            cx,
            reads: Vec::new(),
            parameter_bindings: Vec::new(),
            used_parameters: Vec::new(),
            body_depth: 0,
        }
    }

    /// Performs the `is_reactive_get` operation for this value.
    fn is_reactive_get(&self, expression: &Expr<'_>) -> bool {
        // Prepare the values used by this stage.
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return false;
        };
        if !arguments.is_empty() {
            return false;
        }
        let owner = self.cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Prepare the values used by this stage.
        let Some(method) = self
            .cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        // Update the accumulated analysis state.
        self.cx.tcx.crate_name(method.krate).as_str() == "reactive_graph"
            && self.cx.tcx.item_name(method).as_str() == "get"
            && self
                .cx
                .tcx
                .trait_of_assoc(method)
                .is_some_and(|trait_id| self.cx.tcx.item_name(trait_id).as_str() == "Get")
    }

    /// Performs the `ignored_source` operation for this value.
    fn ignored_source(&self) -> bool {
        self.parameter_bindings
            .iter()
            .all(|binding| !self.used_parameters.contains(binding))
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
        // Reject inputs that do not satisfy this stage.
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
            && self.parameter_bindings.contains(&binding)
            && !self.used_parameters.contains(&binding)
        {
            self.used_parameters.push(binding);
        }

        // Reject inputs that do not satisfy this stage.
        if self.is_reactive_get(expression)
            && let ExprKind::MethodCall(_, receiver, _, _) = expression.kind
            && let ExprKind::Path(path) = receiver.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, receiver.hir_id)
            && !self.reads.iter().any(|read| read.binding == binding)
        {
            self.reads.push(ReactiveRead {
                binding,
                span: expression.span,
            });
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Carries the `ParameterBindings` state used by this analysis.
struct ParameterBindings<'bindings> {
    /// Stores the `bindings` value used by this analysis.
    bindings: &'bindings mut Vec<HirId>,
}

impl<'tcx> Visitor<'tcx> for ParameterBindings<'_> {
    fn visit_pat(&mut self, pattern: &'tcx Pat<'tcx>) {
        if let PatKind::Binding(_, binding, _, _) = pattern.kind {
            self.bindings.push(binding);
        }
        intravisit::walk_pat(self, pattern);
    }
}

/// Source and fetcher closures passed to a resource constructor.
struct ResourceClosures<'tcx> {
    /// Tracked source closure.
    source: &'tcx Expr<'tcx>,
    /// Asynchronous fetcher closure.
    fetcher: &'tcx Expr<'tcx>,
}

/// Carries the `LeptosResourceFetchersRereadingSources` state used by this analysis.
struct LeptosResourceFetchersRereadingSources;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_RESOURCE_FETCHERS_REREADING_SOURCES,
    Warn,
    "rejects Leptos resource fetchers that reread their tracked sources",
    LeptosResourceFetchersRereadingSources
}

impl LeptosResourceFetchersRereadingSources {
    /// Performs the `closures` operation for this value.
    fn closures<'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> Option<ResourceClosures<'tcx>> {
        // Prepare the values used by this stage.
        let ExprKind::Call(callee, arguments) = expression.kind else {
            return None;
        };
        let [source, fetcher] = arguments else {
            return None;
        };

        // Prepare the values used by this stage.
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return None;
        };

        // Reject inputs that do not satisfy this stage.
        if cx.tcx.crate_name(method.krate).as_str() != "leptos_server"
            || cx.tcx.item_name(method).as_str() != "new"
        {
            return None;
        }
        let implementation = cx.tcx.impl_of_assoc(method)?;

        // Prepare the values used by this stage.
        let definition = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()?;

        // Perform the next step of the analysis.
        matches!(
            cx.tcx.item_name(definition.did()).as_str(),
            "Resource" | "ArcResource"
        )
        .then_some(ResourceClosures { source, fetcher })
    }

    /// Performs the `analyze` operation for this value.
    fn analyze<'analysis, 'tcx>(
        cx: &'analysis LateContext<'tcx>,
        closure: &'tcx Expr<'tcx>,
        parameter_collection: ReactiveParameterCollection,
    ) -> Option<ReactiveReads<'analysis, 'tcx>> {
        let ExprKind::Closure(closure) = closure.kind else {
            return None;
        };
        let body = cx.tcx.hir_body(closure.body);
        let mut analysis = ReactiveReads::new(cx);
        if matches!(parameter_collection, ReactiveParameterCollection::Collect) {
            let mut collector = ParameterBindings {
                bindings: &mut analysis.parameter_bindings,
            };
            for parameter in body.params {
                collector.visit_pat(parameter.pat);
            }
        }
        analysis.visit_expr(body.value);
        Some(analysis)
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosResourceFetchersRereadingSources {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Prepare the values used by this stage.
        let Some(ResourceClosures { source, fetcher }) = Self::closures(cx, expression) else {
            return;
        };
        let Some(source) = Self::analyze(cx, source, ReactiveParameterCollection::Ignore) else {
            return;
        };

        // Prepare the values used by this stage.
        let Some(fetcher) = Self::analyze(cx, fetcher, ReactiveParameterCollection::Collect) else {
            return;
        };
        if !fetcher.ignored_source() {
            return;
        }

        // Prepare the values used by this stage.
        let Some(read) = fetcher.reads.iter().find(|read| {
            source
                .reads
                .iter()
                .any(|source_read| source_read.binding == read.binding)
        }) else {
            return;
        };

        // Perform the next step of the analysis.
        Violation {
            owner: expression.hir_id,
            span: read.span,
        }
        .emit(cx);
    }
}
