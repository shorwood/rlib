extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId};
use rustc_lint::LateContext;
use rustc_span::symbol::sym;

use super::parameter_analysis::Parameter;
use super::string_domain_vocabulary::StringDomainSymbolExt;

/// Collects local bindings referenced by one expression.
struct RevalidationBindingCollector<'analysis, 'tcx> {
    /// Compiler context used to resolve paths.
    cx: &'analysis LateContext<'tcx>,
    /// Resolved local binding identities.
    bindings: HashSet<HirId>,
}

impl<'tcx> Visitor<'tcx> for RevalidationBindingCollector<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
        {
            self.bindings.insert(binding);
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Finds returns without descending into nested closures.
struct RevalidationReturnFinder<'analysis, 'tcx> {
    /// Compiler context used to distinguish successful and rejecting return variants.
    cx: &'analysis LateContext<'tcx>,
    /// Whether a rejecting return expression was encountered.
    has_rejection_return: bool,
}

impl RevalidationReturnFinder<'_, '_> {
    /// Returns whether a return value directly constructs standard `Ok` or `Some` success.
    fn is_success_variant(&self, value: &Expr<'_>) -> bool {
        let ExprKind::Call(callee, _) = value.kind else {
            return false;
        };
        let ExprKind::Path(path) = callee.kind else {
            return false;
        };
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            self.cx.qpath_res(&path, callee.hir_id)
        else {
            return false;
        };
        let variant = self.cx.tcx.parent(constructor);
        let owner = self.cx.tcx.parent(variant);
        (self.cx.tcx.item_name(variant).as_str() == "Ok"
            && self.cx.tcx.is_diagnostic_item(sym::Result, owner))
            || (self.cx.tcx.item_name(variant).as_str() == "Some"
                && self.cx.tcx.is_diagnostic_item(sym::Option, owner))
    }
}

impl<'tcx> Visitor<'tcx> for RevalidationReturnFinder<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Ret(value) = expression.kind {
            self.has_rejection_return |= value.is_none_or(|value| !self.is_success_variant(value));
            return;
        }
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Finds parameters used by validation calls or early rejection guards.
struct RevalidationVisitor<'analysis, 'tcx> {
    /// Compiler context used to resolve call targets and bindings.
    cx: &'analysis LateContext<'tcx>,
    /// Textual parameter bindings under analysis.
    parameters: HashSet<HirId>,
    /// Parameters for which invariant establishment was observed.
    revalidated: HashSet<HirId>,
}

impl<'analysis, 'tcx> RevalidationVisitor<'analysis, 'tcx> {
    /// Builds a visitor for the exact textual bindings under analysis.
    fn new(cx: &'analysis LateContext<'tcx>, parameters: &[&Parameter]) -> Self {
        let bindings = parameters.iter().map(|parameter| parameter.hir_id);
        Self {
            cx,
            parameters: bindings.collect(),
            revalidated: HashSet::new(),
        }
    }

    /// Returns whether an expression contains an explicit function return.
    fn contains_return(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = RevalidationReturnFinder {
            cx: self.cx,
            has_rejection_return: false,
        };
        finder.visit_expr(expression);
        finder.has_rejection_return
    }

    /// Records parameter bindings used anywhere inside an expression.
    fn record_bindings(&mut self, expression: &'tcx Expr<'tcx>) {
        let mut collector = RevalidationBindingCollector {
            cx: self.cx,
            bindings: HashSet::new(),
        };
        collector.visit_expr(expression);
        let used_parameters = collector.bindings.intersection(&self.parameters).copied();
        self.revalidated.extend(used_parameters);
    }

    /// Records bindings passed to one invariant-establishing free function.
    fn record_call(&mut self, callee: &'tcx Expr<'tcx>, arguments: &'tcx [Expr<'tcx>]) {
        let ExprKind::Path(path) = callee.kind else {
            return;
        };
        let Res::Def(_, def_id) = self.cx.qpath_res(&path, callee.hir_id) else {
            return;
        };
        let name = self.cx.tcx.item_name(def_id);
        if !name.establishes_domain_invariant() {
            return;
        }
        for argument in arguments {
            self.record_bindings(argument);
        }
    }

    /// Records bindings passed to one invariant-establishing method.
    fn record_method_call(
        &mut self,
        name: rustc_span::Symbol,
        receiver: &'tcx Expr<'tcx>,
        arguments: &'tcx [Expr<'tcx>],
    ) {
        if !name.establishes_domain_invariant() {
            return;
        }
        self.record_bindings(receiver);
        for argument in arguments {
            self.record_bindings(argument);
        }
    }
}

impl<'tcx> Visitor<'tcx> for RevalidationVisitor<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Recognize explicit validation operations and rejection guards.
        match expression.kind {
            ExprKind::Call(callee, arguments) => self.record_call(callee, arguments),
            ExprKind::MethodCall(segment, receiver, arguments, _) => {
                self.record_method_call(segment.ident.name, receiver, arguments);
            }
            ExprKind::If(condition, then, _) if self.contains_return(then) => {
                self.record_bindings(condition);
            }
            _ => {}
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// StringDomainBodyExt: Function-level revalidation query
// -----------------------------------------------------------------------------

/// Revalidation queries colocated with compiler function bodies.
pub(super) trait StringDomainBodyExt {
    /// Returns every textual parameter whose invariant is re-established in this body.
    fn revalidated_bindings<'tcx>(
        &'tcx self,
        cx: &LateContext<'tcx>,
        parameters: &[&Parameter],
    ) -> HashSet<HirId>;
}

impl StringDomainBodyExt for Body<'_> {
    fn revalidated_bindings<'tcx>(
        &'tcx self,
        cx: &LateContext<'tcx>,
        parameters: &[&Parameter],
    ) -> HashSet<HirId> {
        let mut visitor = RevalidationVisitor::new(cx, parameters);
        visitor.visit_expr(self.value);
        visitor.revalidated
    }
}
