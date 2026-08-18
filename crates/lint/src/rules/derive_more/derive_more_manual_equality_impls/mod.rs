extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_ast::LitKind;
use rustc_errors::DiagDecorator;
use rustc_hir::{BinOpKind, Expr, ExprKind, ImplItem, ImplItemKind, Item, ItemKind, Node, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

// -----------------------------------------------------------------------------
// Violation: Derivable manual equality
// -----------------------------------------------------------------------------

/// Structural `PartialEq` implementation awaiting its `Eq` companion.
struct Candidate {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
    /// Number of wrapper fields participating in the equality contract.
    field_count: usize,
}

/// Complete structural equality contract reproducible by `derive_more`.
struct Violation {
    /// Equality implementation proven to compare only corresponding fields.
    candidate: Candidate,
    /// Whether the type also provides the marker `Eq` contract.
    has_eq: bool,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual structural equality for `{}` is derivable",
            self.candidate.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the implementation compares {} corresponding field{} without normalization, adaptation, or external state",
            self.candidate.field_count,
            if self.candidate.field_count == 1 {
                ""
            } else {
                "s"
            }
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        let derives = if self.has_eq {
            "derive_more::PartialEq, derive_more::Eq"
        } else {
            "derive_more::PartialEq"
        };
        Cow::Owned(format!(
            "replace the authored implementation{} with `#[derive({derives})]` and explicit field skips when required",
            if self.has_eq { "s" } else { "" }
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_MANUAL_EQUALITY_IMPLS,
            self.candidate.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.candidate.span,
                    "this equality is pure component comparison",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Identifies a relevant equality trait and its local target type.
struct TraitTarget {
    /// Equality trait implemented by the analyzed item.
    trait_name: &'static str,
    /// Local type receiving the implementation.
    target: LocalDefId,
}

impl TraitTarget {
    /// Recognizes a relevant equality trait implementation.
    fn for_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Generated, non-implementation, or attributed items can carry behavior beyond structural equality.
        if item.span.from_expansion()
            || !matches!(item.kind, ItemKind::Impl(_))
            || !cx.tcx.hir_attrs(item.hir_id()).is_empty()
        {
            return None;
        }
        let trait_ref = cx
            .tcx
            .impl_opt_trait_ref(item.owner_id.def_id)?
            .instantiate_identity();

        // Only core equality traits participate in derive_more equality replacement.
        if cx.tcx.crate_name(trait_ref.def_id.krate).as_str() != "core" {
            return None;
        }

        let name = match cx.tcx.item_name(trait_ref.def_id).as_str() {
            "PartialEq" => "PartialEq",
            "Eq" => "Eq",

            // Other core traits do not establish an equality contract.
            _ => return None,
        };

        // The trait implementation target must be nominal.
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return None;
        };
        Some(Self {
            trait_name: name,
            target: definition.did().as_local()?,
        })
    }
}

/// Describes a field projection and the expression it projects from.
struct FieldAccess<'hir> {
    /// Base expression containing the projected field.
    base: &'hir Expr<'hir>,
    /// Authored field name.
    field: String,
}

impl<'hir> FieldAccess<'hir> {
    /// Recognizes one field projection expression.
    fn from_expr(expression: &'hir Expr<'hir>) -> Option<Self> {
        // Structural equality operands must be direct field projections.
        let ExprKind::Field(base, field) = expression.kind else {
            return None;
        };
        Some(Self {
            base,
            field: field.name.to_string(),
        })
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreManualEqualityImpls: Declarative equality policy
// -----------------------------------------------------------------------------

/// Groups structural `PartialEq` and `Eq` implementations by their wrapper type.
#[derive(Default)]
struct DeriveMoreManualEqualityImpls {
    /// Authored declarations awaiting association with `derive_more` expansions.
    candidates: HashMap<LocalDefId, Candidate>,
    /// Types with an authored `Eq` marker implementation.
    eq_targets: HashSet<LocalDefId>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_EQUALITY_IMPLS,
    Warn,
    "finds structural equality implementations reproducible by derive_more",
    DeriveMoreManualEqualityImpls::default()
}

impl DeriveMoreManualEqualityImpls {
    /// Collects corresponding field pairs joined exclusively by boolean conjunction.
    fn collect_equal_fields(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        left: rustc_hir::HirId,
        right: rustc_hir::HirId,
        fields: &mut HashSet<String>,
    ) -> Option<()> {
        // Structural equality is expressed as a binary relation tree.
        let ExprKind::Binary(operator, lhs, rhs) = expression.kind else {
            return None;
        };

        // Conjunctions preserve structural equality by requiring both operand subtrees.
        if operator.node == BinOpKind::And {
            Self::collect_equal_fields(cx, lhs, left, right, fields)?;
            return Self::collect_equal_fields(cx, rhs, left, right, fields);
        }

        // Only equality leaves can compare corresponding wrapper fields.
        if operator.node != BinOpKind::Eq {
            return None;
        }
        let lhs = FieldAccess::from_expr(lhs)?;
        let rhs = FieldAccess::from_expr(rhs)?;

        // Each leaf must compare one distinct matching field across the two operands.
        if lhs.field != rhs.field
            || !((DirectForwarding::is_binding(cx, lhs.base, left)
                && DirectForwarding::is_binding(cx, rhs.base, right))
                || (DirectForwarding::is_binding(cx, lhs.base, right)
                    && DirectForwarding::is_binding(cx, rhs.base, left)))
            || !fields.insert(lhs.field)
        {
            return None;
        }
        Some(())
    }
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreManualEqualityImpls {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Only recognized equality trait implementations contribute an `Eq` companion.
        let Some(TraitTarget { trait_name, target }) = TraitTarget::for_item(cx, item) else {
            return;
        };

        // Only the marker trait is retained as the `PartialEq` companion.
        if trait_name != "Eq" {
            return;
        }
        self.eq_targets.insert(target);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        // Only exact structural equality methods contribute a derive candidate.
        let Some(StructuralEquality {
            target,
            field_count,
        }) = StructuralEquality::for_item(cx, item)
        else {
            return;
        };

        self.candidates.insert(
            target,
            Candidate {
                span: cx.tcx.def_span(target),
                name: cx.tcx.item_name(target).to_string(),
                field_count,
            },
        );
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut candidates = self.candidates.drain().collect::<Vec<_>>();
        candidates.sort_by_key(|(_, candidate)| candidate.span.lo());
        for (target, candidate) in candidates {
            Violation {
                candidate,
                has_eq: self.eq_targets.contains(&target),
            }
            .emit(cx);
        }
    }
}

/// Summarizes a structural equality implementation.
struct StructuralEquality {
    /// Local type compared by the implementation.
    target: LocalDefId,
    /// Number of distinct fields compared.
    field_count: usize,
}

impl StructuralEquality {
    /// Recognizes a complete structural equality method.
    fn for_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Structural equality analysis applies only to implementation methods.
        let ImplItemKind::Fn(_, body_id) = item.kind else {
            return None;
        };

        // Only authored `eq` methods can implement a structural `PartialEq` contract.
        if item.ident.name.as_str() != "eq" || item.span.from_expansion() {
            return None;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // The method must be enclosed by an implementation item.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return None;
        };

        // The enclosing item must remain an implementation after HIR resolution.
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return None;
        };

        // Attributes may carry equality policy beyond pure field comparison.
        if !cx.tcx.hir_attrs(parent.hir_id()).is_empty()
            || !cx.tcx.hir_attrs(item.hir_id()).is_empty()
        {
            return None;
        }

        // A custom `ne` method carries equality behavior beyond the derived default.
        if implementation_item.items.iter().any(|id| {
            let associated = cx.tcx.hir_impl_item(*id);
            associated.ident.name.as_str() == "ne"
        }) {
            return None;
        }
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();

        // The implementation must be ordinary core `PartialEq` for the same type.
        if cx.tcx.crate_name(trait_ref.def_id.krate).as_str() != "core"
            || cx.tcx.item_name(trait_ref.def_id).as_str() != "PartialEq"
            || trait_ref.self_ty() != trait_ref.args.type_at(1)
        {
            return None;
        }

        // The compared self type must be a nominal struct candidate.
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return None;
        };

        // This derive recognizer supports struct equality only.
        if !definition.is_struct() {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);

        // `PartialEq::eq` must bind both compared operands.
        let [left, right] = body.params else {
            return None;
        };

        // The left operand must retain a direct binding identity.
        let PatKind::Binding(_, left, _, None) = left.pat.kind else {
            return None;
        };

        // The right operand must retain a direct binding identity.
        let PatKind::Binding(_, right, _, None) = right.pat.kind else {
            return None;
        };
        let expression = DirectForwarding::single_body_expression(body.value)?;
        let mut fields = HashSet::new();
        let is_empty_equality = definition.non_enum_variant().fields.is_empty()
            && matches!(expression.kind, ExprKind::Lit(literal) if literal.node == LitKind::Bool(true));

        // Nonempty structs must compare at least one corresponding field.
        if !is_empty_equality {
            DeriveMoreManualEqualityImpls::collect_equal_fields(
                cx,
                expression,
                left,
                right,
                &mut fields,
            )?;
        }
        (is_empty_equality || !fields.is_empty()).then_some(Self {
            target: definition.did().as_local()?,
            field_count: fields.len(),
        })
    }
}
