extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Block, Expr, ExprKind, HirId, Node};
use rustc_lint::LateContext;
use rustc_span::Span;

use super::function_layout_comments::FunctionLayoutCommentSpanExt;
use crate::config::core::FunctionStructureConfig;

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
        // Closures own their returns independently from the surrounding function.
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
}

impl<'analysis, 'tcx> EarlyReturnAnalyzer<'analysis, 'tcx> {
    /// Starts analysis with the compiler context for the function body.
    pub(super) const fn new(cx: &'analysis LateContext<'tcx>) -> Self {
        Self { cx }
    }

    /// Returns whether `statement` precedes later work in its direct block.
    fn block_has_later_entry(block: &Block<'_>, statement: HirId) -> bool {
        block
            .stmts
            .iter()
            .position(|candidate| candidate.hir_id == statement)
            .is_some_and(|index| index + 1 < block.stmts.len() || block.expr.is_some())
    }

    /// Finds the nearest guard whose completion precedes later work in the same function.
    fn boundary(&self, return_: &Expr<'_>) -> Option<Span> {
        let mut guard = None;
        let mut statement = None;
        for (_, node) in self.cx.tcx.hir_parent_iter(return_.hir_id) {
            match node {
                // A closure boundary transfers return ownership away from the analyzed function.
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
                // The first enclosing block with later work establishes a non-terminal guard.
                Node::Block(block)
                    if statement
                        .take()
                        .is_some_and(|statement| Self::block_has_later_entry(block, statement)) =>
                {
                    return guard;
                }
                Node::Item(_) | Node::TraitItem(_) | Node::ImplItem(_) => break,
                _ => {}
            }
        }
        None
    }

    /// Returns whether the controlling condition owns a phase comment.
    fn is_documented(&self, boundary: Span) -> bool {
        boundary.has_phase_comment_candidate_before(
            self.cx,
            FunctionStructureConfig::PHASE_COMMENT_PREFIX,
        )
    }

    /// Finds every unexplained early return in one authored named function body.
    pub(super) fn analyze(self, expression: &'tcx Expr<'tcx>) -> Vec<EarlyReturnFinding> {
        let mut collector = EarlyReturnCollector::default();
        collector.visit_expr(expression);
        let mut findings = Vec::<EarlyReturnFinding>::new();
        for return_ in collector.returns {
            let Some(boundary) = self.boundary(return_) else {
                continue;
            };
            if self.is_documented(boundary)
                || findings
                    .iter()
                    .any(|finding| finding.boundary_span == boundary)
            {
                continue;
            }
            findings.push(EarlyReturnFinding {
                return_span: return_.span,
                boundary_span: boundary,
                help: format!(
                    "add a concise `{} ...` explanation immediately before the controlling condition",
                    FunctionStructureConfig::PHASE_COMMENT_PREFIX
                ),
            });
        }
        findings
    }
}
