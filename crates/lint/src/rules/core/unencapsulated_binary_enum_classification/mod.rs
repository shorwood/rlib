extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::{CtorKind, CtorOf, DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Expr, ExprKind, ItemKind, Node, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

/// Exact variant count governed by the binary-classification policy.
const BINARY_ENUM_VARIANT_COUNT: usize = 2;

// -----------------------------------------------------------------------------
// Violation: Externally owned binary enum classification
// -----------------------------------------------------------------------------

/// Boolean classification that reconstructs a binary enum's complete variant mapping.
struct Violation {
    /// Complete conditional expression that owns the duplicated mapping.
    span: Span,
    /// First selected variant, labeled as one half of the mapping.
    then_span: Span,
    /// Second selected variant, labeled as the other half of the mapping.
    else_span: Span,
    /// Enum whose construction contract is being expressed externally.
    enum_name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this boolean condition classifies every variant of binary enum `{}` outside its inherent impl",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the predicate-to-variant mapping is part of `{}`'s construction contract; keeping it at call sites duplicates its polarity and obscures the canonical conversion",
            self.enum_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "add a semantic constructor on `{}` that accepts the source concept when practical; avoid generic names such as `from_bool` or `from_condition`",
            self.enum_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            UNENCAPSULATED_BINARY_ENUM_CLASSIFICATION,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.then_span, "one outcome is selected here");
                diag.span_label(self.else_span, "the complementary outcome is selected here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// BinaryEnumVariant: Resolved branch outcome
// -----------------------------------------------------------------------------

/// Resolved local unit variant and the enum that defines it.
struct BinaryEnumVariant {
    /// Unit variant selected by one conditional branch.
    variant: LocalDefId,
    /// Binary-enum `candidate` that owns the variant.
    enum_definition: LocalDefId,
}

// -----------------------------------------------------------------------------
// UnencapsulatedBinaryEnumClassification: Construction ownership policy
// -----------------------------------------------------------------------------

/// Finds exhaustive boolean-to-enum mappings outside the enum's inherent implementation.
struct UnencapsulatedBinaryEnumClassification;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub UNENCAPSULATED_BINARY_ENUM_CLASSIFICATION,
    Warn,
    "rejects exhaustive boolean classification of binary enums outside their inherent impl",
    UnencapsulatedBinaryEnumClassification
}

impl LateLintPass<'_> for UnencapsulatedBinaryEnumClassification {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Analyze only authored conditionals with explicit outcomes.
        // Expanded expressions are not authored external classification policy.
        if expression.span.from_expansion() {
            return;
        }

        // Binary classification requires both conditional outcomes.
        let ExprKind::If(_, then_expression, Some(else_expression)) = expression.kind else {
            return;
        };

        // Resolve each pure branch to a local unit-variant constructor.
        let then_expression = Self::branch_outcome(then_expression);
        let else_expression = Self::branch_outcome(else_expression);

        // Generated branch outcomes are not authored variant mappings.
        if then_expression.span.from_expansion() || else_expression.span.from_expansion() {
            return;
        }

        // The true branch must resolve to a local fieldless enum variant.
        let Some(then_variant) = Self::unit_variant(cx, then_expression) else {
            return;
        };

        // The false branch must resolve to a local fieldless enum variant.
        let Some(else_variant) = Self::unit_variant(cx, else_expression) else {
            return;
        };

        // Require the two branches to exhaust one exactly-binary fieldless enum.
        // Repeated or cross-enum outcomes do not define a complete binary mapping.
        if then_variant.enum_definition != else_variant.enum_definition
            || then_variant.variant == else_variant.variant
        {
            return;
        }
        let enum_definition = then_variant.enum_definition;

        // Validate the complete enum shape instead of inferring it from the selected branches.
        let enum_def = cx.tcx.adt_def(enum_definition.to_def_id());

        // Nonbinary or payload-bearing enums require richer construction policy.
        if !enum_def.is_enum()
            || enum_def.variants().len() != BINARY_ENUM_VARIANT_COUNT
            || enum_def
                .variants()
                .iter()
                .any(|variant| !variant.fields.is_empty())
        {
            return;
        }

        // Let the enum's inherent implementation own its canonical classification.
        // Classification is already encapsulated when authored by the enum itself.
        if Self::is_inside_inherent_impl(cx, expression, enum_definition) {
            return;
        }

        // Preserve the complete mapping and its semantic owner for the diagnostic.
        Violation {
            span: expression.span,
            then_span: then_expression.span,
            else_span: else_expression.span,
            enum_name: cx.tcx.item_name(enum_definition.to_def_id()).to_string(),
        }
        .emit(cx);
    }
}

impl UnencapsulatedBinaryEnumClassification {
    /// Peels transparent block and explicit-return wrappers from a branch outcome.
    fn branch_outcome<'hir>(mut expression: &'hir Expr<'hir>) -> &'hir Expr<'hir> {
        expression = expression.peel_blocks();

        // A direct explicit return transparently exposes its selected outcome.
        if let ExprKind::Ret(Some(returned)) = expression.kind {
            return returned.peel_blocks();
        }

        // A bare outcome needs no further block-wrapper peeling.
        let ExprKind::Block(block, _) = expression.kind else {
            return expression;
        };

        // Only a solitary statement can be a transparent return wrapper.
        let [statement] = block.stmts else {
            return expression;
        };

        // Nonexpression statements introduce behavior beyond a return wrapper.
        let (StmtKind::Expr(returned) | StmtKind::Semi(returned)) = statement.kind else {
            return expression;
        };

        // Preserve blocks whose sole statement is not an explicit value return.
        let ExprKind::Ret(Some(returned)) = returned.kind else {
            return expression;
        };
        returned.peel_blocks()
    }

    /// Resolves a pure path expression to its local unit variant and owning enum.
    fn unit_variant(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<BinaryEnumVariant> {
        // Resolve only a direct path to a fieldless variant constructor.
        // Compound expressions do not select one pure enum outcome.
        let ExprKind::Path(ref path) = expression.kind else {
            return None;
        };

        // Exclude paths that are not constant constructors for enum variants.
        let Res::Def(DefKind::Ctor(CtorOf::Variant, CtorKind::Const), constructor) =
            cx.qpath_res(path, expression.hir_id)
        else {
            return None;
        };

        // Walk the local constructor hierarchy to its variant and enum definitions.
        let variant = cx.tcx.opt_local_parent(constructor.as_local()?)?;
        let enum_definition = cx.tcx.opt_local_parent(variant)?;
        Some(BinaryEnumVariant {
            variant,
            enum_definition,
        })
    }

    /// Returns whether an expression is nested in this enum's inherent implementation.
    fn is_inside_inherent_impl(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        enum_definition: LocalDefId,
    ) -> bool {
        cx.tcx
            .hir_parent_iter(expression.hir_id)
            .find_map(|(_, node)| {
                // Only enclosing items can own an implementation context.
                let Node::Item(item) = node else {
                    return None;
                };

                // Continue past enclosing items that are not implementation blocks.
                let ItemKind::Impl(implementation) = item.kind else {
                    return None;
                };
                Some((item, implementation))
            })
            .is_some_and(|(item, implementation)| {
                implementation.of_trait.is_none()
                    && cx
                        .tcx
                        .type_of(item.owner_id)
                        .instantiate_identity()
                        .ty_adt_def()
                        .is_some_and(|definition| {
                            definition.did().as_local() == Some(enum_definition)
                        })
            })
    }
}
