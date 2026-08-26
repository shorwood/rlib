extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::LitKind;
use rustc_errors::DiagDecorator;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, MatchSource, PatExprKind, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::DefId;

use super::utils::view_contract::ViewContract;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// BooleanSelection: Authored boolean view-selection forms
// -----------------------------------------------------------------------------

/// Boolean syntax that selects an authored view.
#[derive(Clone, Copy)]
enum BooleanSelection {
    /// Ordinary two-branch conditional.
    IfElse,
    /// Exhaustive two-arm boolean match.
    Match,
    /// Lazy optional construction through `bool::then`.
    Then,
    /// Eager optional construction through `bool::then_some`.
    ThenSome,
}

impl BooleanSelection {
    /// Returns a diagnostic-facing description of the source form.
    const fn description(self) -> &'static str {
        match self {
            Self::IfElse => "an `if`/`else` expression",
            Self::Match => "a boolean `match` expression",
            Self::Then => "`bool::then`",
            Self::ThenSome => "`bool::then_some`",
        }
    }
}

// -----------------------------------------------------------------------------
// BooleanPattern: Exhaustive boolean match evidence
// -----------------------------------------------------------------------------

/// Boolean pattern shapes accepted in a two-arm partition.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BooleanPattern {
    /// Literal `true`.
    True,
    /// Literal `false`.
    False,
    /// Complementary wildcard after a literal arm.
    Wildcard,
}

// -----------------------------------------------------------------------------
// Violation: Boolean view selection outside Show
// -----------------------------------------------------------------------------

/// Rendered alternatives selected through ordinary Rust boolean control flow.
struct Violation {
    /// Selection expression used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored selection highlighted by the diagnostic.
    span: Span,
    /// Source form retained for the primary message.
    selection: BooleanSelection,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "boolean views are selected through {}",
            self.selection.description()
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Rust control flow hides the condition and alternatives instead of representing them as declarative view structure",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("replace this selection with `<Show when=... fallback=...>`")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MANUAL_BOOLEAN_VIEW_SELECTION,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this boolean selects the rendered view");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosManualBooleanViewSelection: Declarative boolean rendering
// -----------------------------------------------------------------------------

/// Late lint pass that requires boolean views to use Leptos `Show`.
struct LeptosManualBooleanViewSelection;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MANUAL_BOOLEAN_VIEW_SELECTION,
    Warn,
    "requires declarative Show components instead of boolean Rust view selection",
    LeptosManualBooleanViewSelection
}

impl LeptosManualBooleanViewSelection {
    /// Returns whether a condition contains a `let` expression and therefore binds a payload.
    fn contains_let(expression: &Expr<'_>) -> bool {
        match expression.kind {
            ExprKind::Let(_) => true,
            ExprKind::Binary(_, left, right) => {
                Self::contains_let(left) || Self::contains_let(right)
            }
            ExprKind::DropTemps(inner) => Self::contains_let(inner),
            _ => false,
        }
    }

    /// Resolves one literal or wildcard boolean pattern.
    const fn boolean_pattern(pattern: &rustc_hir::Pat<'_>) -> Option<BooleanPattern> {
        match pattern.kind {
            PatKind::Wild => Some(BooleanPattern::Wildcard),
            PatKind::Expr(pattern) => match pattern.kind {
                PatExprKind::Lit {
                    lit,
                    negated: false,
                } => match lit.node {
                    LitKind::Bool(true) => Some(BooleanPattern::True),
                    LitKind::Bool(false) => Some(BooleanPattern::False),
                    _ => None,
                },
                PatExprKind::Lit { negated: true, .. } | PatExprKind::Path(_) => None,
            },
            _ => None,
        }
    }

    /// Returns whether two patterns partition the complete boolean domain.
    const fn is_boolean_partition(first: BooleanPattern, second: BooleanPattern) -> bool {
        matches!(
            (first, second),
            (BooleanPattern::True, BooleanPattern::False)
                | (BooleanPattern::False, BooleanPattern::True)
                | (
                    BooleanPattern::True | BooleanPattern::False,
                    BooleanPattern::Wildcard
                )
        )
    }

    /// Resolves a direct function or associated-function path.
    fn path_definition(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<DefId> {
        // Other expression shapes cannot identify a direct callable definition.
        let ExprKind::Path(path) = expression.kind else {
            return None;
        };

        // Only callable definitions can represent the supported UFCS helpers.
        let Res::Def(DefKind::Fn | DefKind::AssocFn, definition) =
            cx.qpath_res(&path, expression.hir_id)
        else {
            return None;
        };
        Some(definition)
    }

    /// Normalizes method and UFCS calls into their selected definition.
    fn call_definition(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<DefId> {
        match expression.kind {
            ExprKind::MethodCall(..) => {
                let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
                cx.tcx
                    .typeck(owner)
                    .type_dependent_def_id(expression.hir_id)
            }
            ExprKind::Call(callee, _) => Self::path_definition(cx, callee),
            _ => None,
        }
    }

    /// Returns whether a method belongs to the primitive boolean inherent implementation.
    fn is_bool_method(cx: &LateContext<'_>, definition: DefId) -> bool {
        cx.tcx
            .impl_of_assoc(definition)
            .is_some_and(|implementation| {
                cx.tcx
                    .type_of(implementation)
                    .instantiate_identity()
                    .is_bool()
            })
    }

    /// Returns whether an expression span parses as the expected authored syntax.
    fn is_authored(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        predicate: impl FnOnce(&syn::Expr) -> bool,
    ) -> bool {
        cx.sess()
            .source_map()
            .span_to_snippet(expression.span)
            .is_ok_and(|source| {
                syn::parse_str::<syn::Expr>(&source).is_ok_and(|source| predicate(&source))
            })
    }

    /// Classifies a view-producing boolean conditional.
    fn conditional(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<BooleanSelection> {
        // Incomplete conditionals do not select between two rendered alternatives.
        let ExprKind::If(condition, then_branch, Some(else_branch)) = expression.kind else {
            return None;
        };

        // Payload binding and non-view branches belong to different rendering policies.
        if Self::contains_let(condition)
            || !ViewContract::is_view_expression(cx, then_branch)
            || !ViewContract::is_view_expression(cx, else_branch)
            || !Self::is_authored(cx, expression, |source| matches!(source, syn::Expr::If(_)))
        {
            return None;
        }
        Some(BooleanSelection::IfElse)
    }

    /// Returns whether two match arms form a complete boolean view partition.
    fn is_supported_match(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        scrutinee: &Expr<'_>,
        first: &rustc_hir::Arm<'_>,
        second: &rustc_hir::Arm<'_>,
    ) -> bool {
        let owner = cx.tcx.hir_enclosing_body_owner(scrutinee.hir_id);
        let patterns = Self::boolean_pattern(first.pat).zip(Self::boolean_pattern(second.pat));
        cx.tcx.typeck(owner).expr_ty(scrutinee).is_bool()
            && first.guard.is_none()
            && second.guard.is_none()
            && patterns.is_some_and(|(first, second)| Self::is_boolean_partition(first, second))
            && ViewContract::is_view_expression(cx, first.body)
            && ViewContract::is_view_expression(cx, second.body)
            && Self::is_authored(cx, expression, |source| {
                matches!(source, syn::Expr::Match(_))
            })
    }

    /// Classifies an exhaustive two-arm boolean match whose arms yield views.
    fn match_expression(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<BooleanSelection> {
        // Compiler-generated and non-binary matches are outside this authored syntax policy.
        let ExprKind::Match(scrutinee, [first, second], MatchSource::Normal) = expression.kind
        else {
            return None;
        };

        // Only exhaustive, unguarded boolean view partitions correspond to Show.
        Self::is_supported_match(cx, expression, scrutinee, first, second)
            .then_some(BooleanSelection::Match)
    }

    /// Classifies a standard boolean helper that produces an optional view.
    fn helper(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<BooleanSelection> {
        let definition = Self::call_definition(cx, expression)?;

        // Foreign lookalikes and generated call syntax do not express this policy.
        if !Self::is_bool_method(cx, definition)
            || !Self::is_authored(cx, expression, |source| {
                matches!(source, syn::Expr::MethodCall(_) | syn::Expr::Call(_))
            })
        {
            return None;
        }
        let selection = match cx.tcx.item_name(definition).as_str() {
            "then" => Some(BooleanSelection::Then),
            "then_some" => Some(BooleanSelection::ThenSome),
            _ => None,
        }?;
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let output = cx.tcx.typeck(owner).expr_ty(expression);
        let view = ViewContract::option_inner(cx, output)?;
        ViewContract::is_view_type(cx, view).then_some(selection)
    }

    /// Classifies any supported authored boolean view selection.
    fn selection(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<BooleanSelection> {
        Self::conditional(cx, expression)
            .or_else(|| Self::match_expression(cx, expression))
            .or_else(|| Self::helper(cx, expression))
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosManualBooleanViewSelection {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Expressions without one supported semantic shape are unrelated to this lint.
        let Some(selection) = Self::selection(cx, expression) else {
            return;
        };
        Violation {
            owner: expression.hir_id,
            span: expression.span,
            selection,
        }
        .emit(cx);
    }
}
