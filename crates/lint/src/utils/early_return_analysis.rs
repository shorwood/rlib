extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Block, Expr, ExprKind, HirId, Node, Stmt, StmtKind};
use rustc_lint::LateContext;
use rustc_span::Span;

use super::function_layout_comments::has_phase_comment_candidate_before;
use super::function_layout_source::FunctionLayoutPositionExt;
use super::function_structure_config::FunctionStructureConfig;

// -----------------------------------------------------------------------------
// EarlyReturnFinding: Unexplained early-exit boundary
// -----------------------------------------------------------------------------

/// One explicit return that exits before later work without an attached explanation.
pub struct EarlyReturnFinding {
    /// Explicit `return` expression receiving the primary diagnostic.
    pub(crate) return_span: Span,
    /// Preferred guard-level location for the missing explanation.
    pub(crate) boundary_span: Span,
    /// Configuration-aware remediation describing the required comment form.
    pub(crate) help: String,
}

// -----------------------------------------------------------------------------
// EarlyReturnCollector: Authored explicit-return discovery
// -----------------------------------------------------------------------------

/// Finds explicit returns without descending into independently owned closures.
#[derive(Default)]
struct EarlyReturnCollector<'hir> {
    /// Authored return expressions in traversal order.
    returns: Vec<&'hir Expr<'hir>>,
}

impl<'hir> Visitor<'hir> for EarlyReturnCollector<'hir> {
    fn visit_expr(&mut self, expression: &'hir Expr<'hir>) {
        if matches!(expression.kind, ExprKind::Closure(..)) {
            return;
        }
        if matches!(expression.kind, ExprKind::Ret(..)) {
            self.returns.push(expression);
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// EarlyReturnAnalyzer: Guard ownership and comment attachment
// -----------------------------------------------------------------------------

/// Structural analyzer for explicit returns in one named function body.
pub(super) struct EarlyReturnAnalyzer<'analysis, 'tcx> {
    /// Compiler context used for HIR parents and authored source.
    cx: &'analysis LateContext<'tcx>,
    /// Shared function-comment policy.
    config: &'analysis FunctionStructureConfig,
}

impl<'analysis, 'tcx> EarlyReturnAnalyzer<'analysis, 'tcx> {
    /// Starts analysis with the shared function-structure configuration.
    pub(super) const fn new(
        cx: &'analysis LateContext<'tcx>,
        config: &'analysis FunctionStructureConfig,
    ) -> Self {
        Self { cx, config }
    }

    /// Returns whether a direct statement represents already-committed effectful work.
    fn is_effectful(&self, statement: &Stmt<'_>) -> bool {
        let (StmtKind::Expr(expression) | StmtKind::Semi(expression)) = statement.kind else {
            return false;
        };
        match expression.kind {
            ExprKind::Assign(left, ..) | ExprKind::AssignOp(_, left, _) => {
                !matches!(left.kind, ExprKind::Path(path) if matches!(self.cx.qpath_res(&path, left.hir_id), Res::Local(_)))
            }
            ExprKind::InlineAsm(..) | ExprKind::Yield(..) => true,
            _ => false,
        }
    }

    /// Returns whether `statement` follows effectful work and precedes a later direct entry.
    fn block_surrounds_statement(&self, block: &Block<'_>, statement: HirId) -> bool {
        block
            .stmts
            .iter()
            .position(|candidate| candidate.hir_id == statement)
            .is_some_and(|index| {
                block.stmts[..index]
                    .iter()
                    .any(|statement| self.is_effectful(statement))
                    && (index + 1 < block.stmts.len() || block.expr.is_some())
            })
    }

    /// Finds the nearest guard whose completion precedes later work in the same function.
    fn boundary(&self, return_: &Expr<'_>) -> Option<Span> {
        let mut guard = None;
        let mut statement = None;
        for (_, node) in self.cx.tcx.hir_parent_iter(return_.hir_id) {
            match node {
                Node::Expr(expression) if matches!(expression.kind, ExprKind::Closure(..)) => {
                    return None;
                }
                Node::Expr(expression) if matches!(expression.kind, ExprKind::If(..)) => {
                    guard.get_or_insert(expression.span);
                }
                Node::Arm(arm) => {
                    guard.get_or_insert(arm.span);
                }
                Node::LetStmt(local) if local.els.is_some() => {
                    guard.get_or_insert(local.span);
                }
                Node::Stmt(candidate) => statement = Some(candidate.hir_id),
                Node::Block(block)
                    if statement.take().is_some_and(|statement| {
                        self.block_surrounds_statement(block, statement)
                    }) =>
                {
                    return Some(guard.unwrap_or(return_.span));
                }
                Node::Item(_) | Node::TraitItem(_) | Node::ImplItem(_) => break,
                _ => {}
            }
        }
        None
    }

    /// Returns whether either the preferred guard or fallback return owns a phase comment.
    fn is_documented(&self, return_: Span, boundary: Span) -> bool {
        has_phase_comment_candidate_before(self.cx, boundary, &self.config.phase_comment_prefix)
            || (boundary != return_
                && has_phase_comment_candidate_before(
                    self.cx,
                    return_,
                    &self.config.phase_comment_prefix,
                ))
    }

    /// Returns whether the exit bypasses enough source to make its effect nonlocal.
    fn has_substantial_continuation(&self, body: Span, boundary: Span) -> bool {
        body.hi()
            .source_line(self.cx)
            .saturating_sub(boundary.hi().source_line(self.cx))
            > self.config.max_phase_lines
    }

    /// Finds every unexplained early return in one authored named function body.
    pub(super) fn analyze(self, expression: &'tcx Expr<'tcx>) -> Vec<EarlyReturnFinding> {
        let mut collector = EarlyReturnCollector::default();
        collector.visit_expr(expression);
        collector
            .returns
            .into_iter()
            .filter_map(|return_| {
                let boundary = self.boundary(return_)?;
                if !self.has_substantial_continuation(expression.span, boundary)
                    || self.is_documented(return_.span, boundary)
                {
                    return None;
                }
                Some(EarlyReturnFinding {
                    return_span: return_.span,
                    boundary_span: boundary,
                    help: format!(
                        "add a concise `{} ...` explanation immediately before the {}",
                        self.config.phase_comment_prefix,
                        if boundary == return_.span {
                            "return"
                        } else {
                            "guard"
                        }
                    ),
                })
            })
            .collect()
    }
}
