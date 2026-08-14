extern crate rustc_hir;
extern crate rustc_lint;

use std::collections::HashSet;

use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId, Pat, PatKind, Stmt, StmtKind};
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
            bindings: HashSet::from([binding]),
            result_expressions: TextResultCollector::collect(body.value),
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
    /// Receiver-derived bindings being searched for.
    bindings: &'analysis HashSet<HirId>,
    /// Whether traversal reached the binding.
    has_found: bool,
}

impl<'tcx> Visitor<'tcx> for TextInputUseFinder<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && matches!(self.cx.qpath_res(&path, expression.hir_id), Res::Local(binding) if self.bindings.contains(&binding))
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
    /// Bindings transitively derived from the authored receiver or parameter.
    bindings: HashSet<HirId>,
    /// Tail values and explicit-return values that determine the accessor result.
    result_expressions: HashSet<HirId>,
    /// Facts accumulated while traversing the body.
    evidence: TextBodyEvidence,
}

impl<'tcx> TextBodyAnalyzer<'_, 'tcx> {
    /// Returns whether one expression references the authored input binding.
    fn expression_uses_input(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = TextInputUseFinder {
            cx: self.cx,
            bindings: &self.bindings,
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
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = statement.kind
            && let Some(initializer) = local.init
        {
            let derives_from_input = self.expression_uses_input(initializer);
            self.visit_expr(initializer);
            if derives_from_input {
                self.record_bindings(local.pat);
            }
            if let Some(else_block) = local.els {
                self.visit_block(else_block);
            }
            return;
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.result_expressions.contains(&expression.hir_id)
            && self.expression_uses_input(expression)
        {
            self.evidence.has_input_use = true;
        }

        let target = match expression.kind {
            ExprKind::MethodCall(_, receiver, _, _) => self.method_target(expression, receiver),
            ExprKind::Call(callee, arguments) => self.function_target(callee, arguments),
            _ => None,
        };
        if self.result_expressions.contains(&expression.hir_id)
            && target.is_some_and(|target| {
                self.cx
                    .tcx
                    .def_path_str(target)
                    .ends_with("::ToString::to_string")
            })
        {
            self.evidence.has_display_delegation = true;
        }

        if let ExprKind::Assign(left, right, _) = expression.kind {
            let derives_from_input = self.expression_uses_input(right);
            self.visit_expr(right);
            if let ExprKind::Path(path) = left.kind
                && let Res::Local(binding) = self.cx.qpath_res(&path, left.hir_id)
            {
                if derives_from_input {
                    self.bindings.insert(binding);
                } else {
                    self.bindings.remove(&binding);
                }
            }
            self.visit_expr(left);
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

impl TextBodyAnalyzer<'_, '_> {
    /// Adds every plain binding introduced by a receiver-derived pattern.
    fn record_bindings(&mut self, pattern: &Pat<'_>) {
        /// Collects binding identities from one pattern.
        struct BindingCollector<'set> {
            bindings: &'set mut HashSet<HirId>,
        }
        impl<'tcx> Visitor<'tcx> for BindingCollector<'_> {
            fn visit_pat(&mut self, pattern: &'tcx Pat<'tcx>) {
                if let PatKind::Binding(_, binding, _, _) = pattern.kind {
                    self.bindings.insert(binding);
                }
                intravisit::walk_pat(self, pattern);
            }
        }
        BindingCollector {
            bindings: &mut self.bindings,
        }
        .visit_pat(pattern);
    }
}

// -----------------------------------------------------------------------------
// TextResultCollector: Returned accessor values
// -----------------------------------------------------------------------------

/// Finds tail values and explicit returns without entering dormant closures.
#[derive(Default)]
struct TextResultCollector {
    expressions: HashSet<HirId>,
}

impl TextResultCollector {
    /// Collects every expression that directly supplies a function result.
    fn collect(expression: &Expr<'_>) -> HashSet<HirId> {
        let mut collector = Self::default();
        collector.visit_result_expr(expression);
        collector.expressions
    }

    /// Follows blocks and branches to the values they return.
    fn visit_result_expr<'tcx>(&mut self, expression: &'tcx Expr<'tcx>) {
        match expression.kind {
            ExprKind::Block(block, _) => {
                for statement in block.stmts {
                    self.visit_stmt(statement);
                }
                if let Some(tail) = block.expr {
                    self.visit_result_expr(tail);
                }
            }
            ExprKind::If(condition, then_expression, else_expression) => {
                self.visit_expr(condition);
                self.visit_result_expr(then_expression);
                if let Some(else_expression) = else_expression {
                    self.visit_result_expr(else_expression);
                }
            }
            ExprKind::Match(scrutinee, arms, _) => {
                self.visit_expr(scrutinee);
                for arm in arms {
                    if let Some(guard) = arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_result_expr(arm.body);
                }
            }
            ExprKind::Ret(Some(value)) | ExprKind::DropTemps(value) => {
                self.visit_result_expr(value);
            }
            _ => {
                self.expressions.insert(expression.hir_id);
                self.visit_expr(expression);
            }
        }
    }
}

impl<'tcx> Visitor<'tcx> for TextResultCollector {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        match expression.kind {
            ExprKind::Ret(Some(value)) => self.visit_result_expr(value),
            ExprKind::Closure(_) => {}
            _ => intravisit::walk_expr(self, expression),
        }
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}
