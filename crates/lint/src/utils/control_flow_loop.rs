extern crate rustc_hir;
extern crate rustc_lint;

use rustc_hir::{Block, Expr, ExprKind, HirId, MatchSource, Node};
use rustc_lint::{LateContext, LintContext};

// -----------------------------------------------------------------------------
// ControlFlowLoop: Direct loop body recognition
// -----------------------------------------------------------------------------

/// Distinguishes a loop's direct body block from blocks nested inside its tail.
fn ends_with_block(cx: &LateContext<'_>, expression: &Expr<'_>, block: &Block<'_>) -> bool {
    let source_map = cx.sess().source_map();
    let block_end = source_map.lookup_char_pos(block.span.hi()).line;
    let loop_end = source_map.lookup_char_pos(expression.span.hi()).line;
    block_end == loop_end
}

/// Classifies expression parents that establish a block's relationship to a loop.
fn expression_parent_relation(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    block: &Block<'_>,
) -> Option<bool> {
    match expression.kind {
        ExprKind::Loop(..) => Some(ends_with_block(cx, expression, block)),
        ExprKind::Closure(..)
        | ExprKind::If(..)
        | ExprKind::Match(_, _, MatchSource::Normal | MatchSource::Postfix) => Some(false),
        _ => None,
    }
}

/// Classifies whether a parent proves or disproves that `block` is a direct loop body.
fn parent_relation(cx: &LateContext<'_>, parent: HirId, block: &Block<'_>) -> Option<bool> {
    match cx.tcx.hir_node(parent) {
        Node::Expr(expression) => expression_parent_relation(cx, expression, block),
        Node::Item(_) | Node::TraitItem(_) | Node::ImplItem(_) | Node::Crate(_) => Some(false),
        _ => None,
    }
}

/// Walks transparent HIR parents to determine whether `block` is a loop body.
pub(super) fn is_direct_body(cx: &LateContext<'_>, block: &Block<'_>) -> bool {
    let mut current = block.hir_id;
    loop {
        let parent = cx.tcx.parent_hir_id(current);
        if parent == current {
            return false;
        }
        if let Some(is_direct) = parent_relation(cx, parent, block) {
            return is_direct;
        }
        current = parent;
    }
}
