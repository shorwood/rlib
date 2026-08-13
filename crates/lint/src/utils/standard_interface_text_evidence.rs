extern crate rustc_hir;
extern crate rustc_lint;

use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId};
use rustc_lint::LateContext;

// -----------------------------------------------------------------------------
// TextBodyEvidence: Source use and standard delegation facts
// -----------------------------------------------------------------------------

/// Complete source-use and standard-delegation facts for one text-producing body.
pub(super) struct TextBodyEvidence {
    /// Whether the body reads the authored receiver or parameter.
    pub(super) has_input_use: bool,
    /// Whether the body resolves a direct `ToString` delegation.
    pub(super) has_display_delegation: bool,
}

impl TextBodyEvidence {
    /// Analyzes one text-producing body for source use and standard delegation.
    pub(super) fn analyze<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        binding: HirId,
    ) -> Self {
        let mut analyzer = TextBodyAnalyzer {
            cx,
            binding,
            evidence: Self {
                has_input_use: false,
                has_display_delegation: false,
            },
        };
        analyzer.visit_expr(body.value);
        analyzer.evidence
    }
}

// -----------------------------------------------------------------------------
// TextInputUseFinder: Authored input provenance
// -----------------------------------------------------------------------------

/// Finds the authored input binding inside one expression subtree.
struct TextInputUseFinder<'analysis, 'tcx> {
    /// Compiler context used to resolve local paths.
    cx: &'analysis LateContext<'tcx>,
    /// Authored binding being searched for.
    binding: HirId,
    /// Whether traversal reached the binding.
    has_found: bool,
}

impl<'tcx> Visitor<'tcx> for TextInputUseFinder<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && self.cx.qpath_res(&path, expression.hir_id) == Res::Local(self.binding)
        {
            self.has_found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// TextBodyAnalyzer: Text body traversal
// -----------------------------------------------------------------------------

/// Finds input provenance and direct calls into `ToString`.
struct TextBodyAnalyzer<'analysis, 'tcx> {
    /// Compiler context used to resolve paths and method calls.
    cx: &'analysis LateContext<'tcx>,
    /// Authored receiver or parameter binding.
    binding: HirId,
    /// Facts accumulated while traversing the body.
    evidence: TextBodyEvidence,
}

impl<'tcx> TextBodyAnalyzer<'_, 'tcx> {
    /// Returns whether one expression references the authored input binding.
    fn expression_uses_input(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = TextInputUseFinder {
            cx: self.cx,
            binding: self.binding,
            has_found: false,
        };
        finder.visit_expr(expression);
        finder.has_found
    }

    /// Resolves a method target when its receiver derives from the authored input.
    fn method_target(
        &self,
        expression: &'tcx Expr<'tcx>,
        receiver: &'tcx Expr<'tcx>,
    ) -> Option<DefId> {
        if !self.expression_uses_input(receiver) {
            return None;
        }
        self.cx
            .typeck_results()
            .type_dependent_def_id(expression.hir_id)
    }

    /// Resolves a free function target when an argument derives from the authored input.
    fn function_target(
        &self,
        callee: &'tcx Expr<'tcx>,
        arguments: &'tcx [Expr<'tcx>],
    ) -> Option<DefId> {
        if !arguments
            .iter()
            .any(|argument| self.expression_uses_input(argument))
        {
            return None;
        }
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        self.cx.qpath_res(&path, callee.hir_id).opt_def_id()
    }
}

impl<'tcx> Visitor<'tcx> for TextBodyAnalyzer<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && self.cx.qpath_res(&path, expression.hir_id) == Res::Local(self.binding)
        {
            self.evidence.has_input_use = true;
        }

        let target = match expression.kind {
            ExprKind::MethodCall(_, receiver, _, _) => self.method_target(expression, receiver),
            ExprKind::Call(callee, arguments) => self.function_target(callee, arguments),
            _ => None,
        };
        if target.is_some_and(|target| {
            self.cx
                .tcx
                .def_path_str(target)
                .ends_with("::ToString::to_string")
        }) {
            self.evidence.has_display_delegation = true;
        }
        intravisit::walk_expr(self, expression);
    }
}
