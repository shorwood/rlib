extern crate rustc_hir;
extern crate rustc_lint;

use rustc_hir::{Arm, Expr, ExprKind, Node};
use rustc_lint::LateContext;

use super::function_layout_analysis::function_layout_code_line_count;

// -----------------------------------------------------------------------------
// ControlFlowMetrics: Match arm and method chain measurements
// -----------------------------------------------------------------------------

/// Counts authored code lines in one match arm body without counting its braces.
pub(super) fn match_arm_lines(cx: &LateContext<'_>, arm: &Arm<'_>) -> usize {
    if let ExprKind::Block(block, _) = arm.body.kind {
        block
            .stmts
            .iter()
            .map(|statement| function_layout_code_line_count(cx, statement.span))
            .sum::<usize>()
            + block.expr.map_or(0, |expression| {
                function_layout_code_line_count(cx, expression.span)
            })
    } else {
        function_layout_code_line_count(cx, arm.body.span)
    }
}

/// Counts consecutive method calls by following receiver expressions inward.
pub(super) const fn method_chain_length(mut expression: &Expr<'_>) -> usize {
    let mut calls = 0;
    while let ExprKind::MethodCall(_, receiver, _, _) = expression.kind {
        calls += 1;
        expression = receiver;
    }
    calls
}

/// Returns whether an expression is the receiver of a surrounding method call.
pub(super) fn is_parent_method_receiver(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
    matches!(
        cx.tcx.parent_hir_node(expression.hir_id),
        Node::Expr(Expr {
            kind: ExprKind::MethodCall(_, receiver, _, _),
            ..
        }) if receiver.hir_id == expression.hir_id
    )
}
