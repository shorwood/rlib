extern crate rustc_hir;
extern crate rustc_lint;

use rustc_hir::{Arm, Expr, ExprKind, Node};
use rustc_lint::LateContext;

use super::function_layout_analysis::FunctionLayoutSpanExt;

// -----------------------------------------------------------------------------
// ControlFlowMetrics: Match arm and method chain measurements
// -----------------------------------------------------------------------------

/// Match-arm measurements colocated with compiler arms.
pub(super) trait ControlFlowArmExt {
    /// Counts authored code lines in this match arm body without counting its braces.
    fn code_lines(&self, cx: &LateContext<'_>) -> usize;
}

impl ControlFlowArmExt for Arm<'_> {
    fn code_lines(&self, cx: &LateContext<'_>) -> usize {
        if let ExprKind::Block(block, _) = self.body.kind {
            // Count direct statements without charging the arm's braces.
            let statement_lines = block
                .stmts
                .iter()
                .map(|statement| statement.span.code_line_count(cx))
                .sum::<usize>();

            // Add the optional tail expression as an independent authored line range.
            let tail_lines = block
                .expr
                .map_or(0, |expression| expression.span.code_line_count(cx));
            statement_lines + tail_lines
        } else {
            self.body.span.code_line_count(cx)
        }
    }
}

/// Method-chain queries colocated with compiler expressions.
pub(super) trait ControlFlowExpressionExt {
    /// Counts consecutive method calls by following receiver expressions inward.
    fn method_chain_length(&self) -> usize;
    /// Returns whether this expression is the receiver of a surrounding method call.
    fn is_parent_method_receiver(&self, cx: &LateContext<'_>) -> bool;
}

impl ControlFlowExpressionExt for Expr<'_> {
    fn method_chain_length(&self) -> usize {
        let mut expression = self;
        let mut calls = 0;
        while let ExprKind::MethodCall(_, receiver, _, _) = expression.kind {
            calls += 1;
            expression = receiver;
        }
        calls
    }

    fn is_parent_method_receiver(&self, cx: &LateContext<'_>) -> bool {
        matches!(
            cx.tcx.parent_hir_node(self.hir_id),
            Node::Expr(Expr {
                kind: ExprKind::MethodCall(_, receiver, _, _),
                ..
            }) if receiver.hir_id == self.hir_id
        )
    }
}
