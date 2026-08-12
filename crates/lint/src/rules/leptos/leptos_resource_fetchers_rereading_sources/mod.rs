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

// -----------------------------------------------------------------------------
// Violation: Resource input bypass diagnostic
// -----------------------------------------------------------------------------

struct Violation {
    owner: HirId,
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

// -----------------------------------------------------------------------------
// ReactiveReads: Closure dependency analysis
// -----------------------------------------------------------------------------

struct ReactiveReads<'analysis, 'tcx> {
    cx: &'analysis LateContext<'tcx>,
    reads: Vec<(HirId, Span)>,
    parameter_bindings: Vec<HirId>,
    used_parameters: Vec<HirId>,
    body_depth: u8,
}

impl<'analysis, 'tcx> ReactiveReads<'analysis, 'tcx> {
    const fn new(cx: &'analysis LateContext<'tcx>) -> Self {
        Self {
            cx,
            reads: Vec::new(),
            parameter_bindings: Vec::new(),
            used_parameters: Vec::new(),
            body_depth: 0,
        }
    }

    fn is_reactive_get(&self, expression: &Expr<'_>) -> bool {
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return false;
        };
        if !arguments.is_empty() {
            return false;
        }
        let owner = self.cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let Some(method) = self
            .cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };
        self.cx.tcx.crate_name(method.krate).as_str() == "reactive_graph"
            && self.cx.tcx.item_name(method).as_str() == "get"
            && self
                .cx
                .tcx
                .trait_of_assoc(method)
                .is_some_and(|trait_id| self.cx.tcx.item_name(trait_id).as_str() == "Get")
    }

    fn ignored_source(&self) -> bool {
        self.parameter_bindings
            .iter()
            .all(|binding| !self.used_parameters.contains(binding))
    }
}

impl<'tcx> Visitor<'tcx> for ReactiveReads<'_, 'tcx> {
    fn visit_nested_body(&mut self, body_id: BodyId) {
        if self.body_depth >= 2 {
            return;
        }
        self.body_depth += 1;
        self.visit_body(self.cx.tcx.hir_body(body_id));
        self.body_depth -= 1;
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
            && self.parameter_bindings.contains(&binding)
            && !self.used_parameters.contains(&binding)
        {
            self.used_parameters.push(binding);
        }
        if self.is_reactive_get(expression)
            && let ExprKind::MethodCall(_, receiver, _, _) = expression.kind
            && let ExprKind::Path(path) = receiver.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, receiver.hir_id)
            && !self.reads.iter().any(|(existing, _)| *existing == binding)
        {
            self.reads.push((binding, expression.span));
        }
        intravisit::walk_expr(self, expression);
    }
}

struct ParameterBindings<'bindings> {
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

// -----------------------------------------------------------------------------
// LeptosResourceFetchersRereadingSources: Resource input policy
// -----------------------------------------------------------------------------

struct LeptosResourceFetchersRereadingSources;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_RESOURCE_FETCHERS_REREADING_SOURCES,
    Warn,
    "rejects Leptos resource fetchers that reread their tracked sources",
    LeptosResourceFetchersRereadingSources
}

impl LeptosResourceFetchersRereadingSources {
    fn closures<'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> Option<(&'tcx Expr<'tcx>, &'tcx Expr<'tcx>)> {
        let ExprKind::Call(callee, arguments) = expression.kind else {
            return None;
        };
        let [source, fetcher] = arguments else {
            return None;
        };
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return None;
        };
        if cx.tcx.crate_name(method.krate).as_str() != "leptos_server"
            || cx.tcx.item_name(method).as_str() != "new"
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
        .then_some((source, fetcher))
    }

    fn analyze<'analysis, 'tcx>(
        cx: &'analysis LateContext<'tcx>,
        closure: &'tcx Expr<'tcx>,
        collect_parameters: bool,
    ) -> Option<ReactiveReads<'analysis, 'tcx>> {
        let ExprKind::Closure(closure) = closure.kind else {
            return None;
        };
        let body = cx.tcx.hir_body(closure.body);
        let mut analysis = ReactiveReads::new(cx);
        if collect_parameters {
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
        let Some((source, fetcher)) = Self::closures(cx, expression) else {
            return;
        };
        let Some(source) = Self::analyze(cx, source, false) else {
            return;
        };
        let Some(fetcher) = Self::analyze(cx, fetcher, true) else {
            return;
        };
        if !fetcher.ignored_source() {
            return;
        }
        let Some((_, span)) = fetcher.reads.iter().find(|(binding, _)| {
            source
                .reads
                .iter()
                .any(|(source_binding, _)| source_binding == binding)
        }) else {
            return;
        };
        Violation {
            owner: expression.hir_id,
            span: *span,
        }
        .emit(cx);
    }
}
