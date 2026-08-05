extern crate rustc_ast;
extern crate rustc_errors;

use rustc_ast::ast::{BinOpKind, Expr, ExprKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};

// -----------------------------------------------------------------------------
// UnparenthesizedMixedBooleanOperators: Boolean grouping policy
// -----------------------------------------------------------------------------

/// Checks authored boolean syntax before parentheses are removed during lowering.
struct UnparenthesizedMixedBooleanOperators;

dylint_linting::impl_early_lint! {
    /// ### What it does
    ///
    /// Finds boolean expressions that directly mix `&&` and `||` without parentheses expressing
    /// their intended grouping. Chains containing only one of the operators remain valid.
    ///
    /// ### Why is this bad?
    ///
    /// Rust gives `&&` higher precedence than `||`, but relying on that fact makes subtle conditions
    /// easy to misread and edit incorrectly. Explicit groups communicate whether one requirement
    /// governs every alternative or only the neighboring expression.
    ///
    /// For example, this expression can easily be mistaken for requiring `is_final` in both cases:
    ///
    /// ```rust
    /// # let (is_final, left_matches, right_matches) = (true, true, true);
    /// let accepted = is_final && left_matches || right_matches;
    /// # let _ = accepted;
    /// ```
    ///
    /// Parenthesize the alternatives to state that intention directly:
    ///
    /// ```rust
    /// # let (is_final, left_matches, right_matches) = (true, true, true);
    /// let accepted = is_final && (left_matches || right_matches);
    /// # let _ = accepted;
    /// ```
    pub UNPARENTHESIZED_MIXED_BOOLEAN_OPERATORS,
    Warn,
    "rejects boolean expressions that mix && and || without explicit grouping",
    UnparenthesizedMixedBooleanOperators
}

impl EarlyLintPass for UnparenthesizedMixedBooleanOperators {
    fn check_expr(&mut self, cx: &EarlyContext<'_>, expression: &Expr) {
        // Retain authored binary expressions whose direct operand mixes boolean operators.
        if expression.span.from_expansion() {
            return;
        }
        let ExprKind::Binary(operator, left, right) = &expression.kind else {
            return;
        };
        if !Self::is_boolean(operator.node)
            || ![left, right]
                .into_iter()
                .any(|operand| Self::has_opposite_operator(operand, operator.node))
        {
            return;
        }

        // Report the complete ambiguous expression with explicit grouping guidance.
        cx.emit_span_lint(
            UNPARENTHESIZED_MIXED_BOOLEAN_OPERATORS,
            expression.span,
            DiagDecorator(|diag| {
                diag.primary_message("this boolean expression mixes `&&` and `||` without explicit grouping");
                diag.help("parenthesize the intended boolean groups instead of relying on operator precedence");
            }),
        );
    }
}

impl UnparenthesizedMixedBooleanOperators {
    /// Returns whether an operator participates in boolean short-circuiting.
    const fn is_boolean(operator: BinOpKind) -> bool {
        matches!(operator, BinOpKind::And | BinOpKind::Or)
    }

    /// Returns whether a direct operand uses the other boolean operator.
    const fn has_opposite_operator(expression: &Expr, parent: BinOpKind) -> bool {
        let ExprKind::Binary(operator, ..) = expression.kind else {
            return false;
        };
        matches!(
            (parent, operator.node),
            (BinOpKind::And, BinOpKind::Or) | (BinOpKind::Or, BinOpKind::And)
        )
    }
}
