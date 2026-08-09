extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{BinOpKind, Expr, ExprKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Ambiguous boolean grouping diagnostic
// -----------------------------------------------------------------------------

/// Authored boolean expression whose direct operand uses the opposite operator.
struct Violation {
    /// Complete mixed expression span used as the primary diagnostic location.
    span: Span,
    /// Direct mixed operand span shown as the grouping site.
    mixed_operand_span: Span,
    /// Outer boolean operator.
    outer_operator: &'static str,
    /// Nested opposing boolean operator.
    inner_operator: &'static str,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this boolean expression mixes `{}` and `{}` without explicit grouping",
            self.outer_operator, self.inner_operator
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Rust precedence determines how `{}` binds inside `{}`, but the authored syntax does not make that intention visually explicit",
            self.inner_operator, self.outer_operator
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "parenthesize the `{}` operand to state the intended boolean group",
            self.inner_operator
        ))
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            UNPARENTHESIZED_MIXED_BOOLEAN_OPERATORS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.mixed_operand_span, "make this nested group explicit");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

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
    /// Rust gives `&&` higher precedence than `||`, but relying on that fact makes subtle
    /// conditions easy to misread and edit incorrectly. Explicit groups communicate whether one
    /// requirement governs every alternative or only the neighboring expression.
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

        // Resolve the authored outer boolean expression before locating its mixed operand.
        let ExprKind::Binary(operator, left, right) = &expression.kind else {
            return;
        };
        if !Self::is_boolean(operator.node) {
            return;
        }

        // Identify the direct operand whose opposing operator needs explicit grouping.
        let Some(mixed_operand) = [left, right]
            .into_iter()
            .find(|operand| Self::has_opposite_operator(operand, operator.node))
        else {
            return;
        };

        // Report the complete ambiguous expression with explicit grouping guidance.
        Violation {
            span: expression.span,
            mixed_operand_span: mixed_operand.span,
            outer_operator: Self::operator_text(operator.node),
            inner_operator: Self::operator_text(Self::opposite(operator.node)),
        }
        .emit(cx);
    }
}

impl UnparenthesizedMixedBooleanOperators {
    /// Returns whether an operator participates in boolean short-circuiting.
    const fn is_boolean(operator: BinOpKind) -> bool {
        matches!(operator, BinOpKind::And | BinOpKind::Or)
    }

    /// Returns the opposing short-circuit operator for a validated boolean operator.
    const fn opposite(operator: BinOpKind) -> BinOpKind {
        match operator {
            BinOpKind::And => BinOpKind::Or,
            BinOpKind::Or => BinOpKind::And,
            _ => unreachable!(),
        }
    }

    /// Renders a validated short-circuit operator in authored Rust syntax.
    const fn operator_text(operator: BinOpKind) -> &'static str {
        match operator {
            BinOpKind::And => "&&",
            BinOpKind::Or => "||",
            _ => unreachable!(),
        }
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
