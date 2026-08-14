extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::{Arm, Expr, ExprKind, Node};
use rustc_lint::LateContext;
use rustc_span::BytePos;

use super::function_layout_analysis::FunctionLayoutAnalyzerSpanExt;

// -----------------------------------------------------------------------------
// ControlFlow: Match arm and method chain measurements
// -----------------------------------------------------------------------------

/// Match-arm measurements colocated with compiler arms.
pub(super) trait ControlFlowArmExt {
    /// Counts authored code lines in this match arm body without counting its braces.
    fn code_lines(&self, cx: &LateContext<'_>) -> usize;
}

impl ControlFlowArmExt for Arm<'_> {
    fn code_lines(&self, cx: &LateContext<'_>) -> usize {
        if let ExprKind::Block(block, _) = self.body.kind {
            // Measure the block interior once so several statements sharing a physical line do
            // not count that line repeatedly. Removing the brace bytes also keeps brace-only
            // lines outside the authored-code budget.
            let interior = block
                .span
                .with_lo(block.span.lo() + BytePos(1))
                .with_hi(block.span.hi() - BytePos(1));
            interior.code_line_count(cx)
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
