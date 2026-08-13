extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{BinOpKind, Expr, ExprKind, ImplItem, ImplItemKind, Item, ItemKind, Node, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

struct Candidate {
    span: Span,
    name: String,
    field_count: usize,
}

struct Violation {
    candidate: Candidate,
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

#[derive(Default)]
struct DeriveMoreManualEqualityImpls {
    candidates: HashMap<LocalDefId, Candidate>,
    eq_targets: HashSet<LocalDefId>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_EQUALITY_IMPLS,
    Warn,
    "finds structural equality implementations reproducible by derive_more",
    DeriveMoreManualEqualityImpls::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreManualEqualityImpls {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let Some((trait_name, target)) = trait_target(cx, item) else {
            return;
        };
        if trait_name == "Eq" {
            self.eq_targets.insert(target);
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        let Some((target, field_count)) = structural_equality(cx, item) else {
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
        for (target, candidate) in self.candidates.drain() {
            Violation {
                candidate,
                has_eq: self.eq_targets.contains(&target),
            }
            .emit(cx);
        }
    }
}

fn trait_target(cx: &LateContext<'_>, item: &Item<'_>) -> Option<(&'static str, LocalDefId)> {
    if item.span.from_expansion() || !matches!(item.kind, ItemKind::Impl(_)) {
        return None;
    }
    let trait_ref = cx
        .tcx
        .impl_opt_trait_ref(item.owner_id.def_id)?
        .instantiate_identity();
    if cx.tcx.crate_name(trait_ref.def_id.krate).as_str() != "core" {
        return None;
    }
    let name = match cx.tcx.item_name(trait_ref.def_id).as_str() {
        "PartialEq" => "PartialEq",
        "Eq" => "Eq",
        _ => return None,
    };
    let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
        return None;
    };
    Some((name, definition.did().as_local()?))
}

fn structural_equality(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<(LocalDefId, usize)> {
    let ImplItemKind::Fn(_, body_id) = item.kind else {
        return None;
    };
    if item.ident.name.as_str() != "eq" || item.span.from_expansion() {
        return None;
    }
    let implementation = cx.tcx.local_parent(item.owner_id.def_id);
    let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
        return None;
    };
    let ItemKind::Impl(implementation_item) = parent.kind else {
        return None;
    };
    if implementation_item.items.iter().any(|id| {
        let associated = cx.tcx.hir_impl_item(*id);
        associated.ident.name.as_str() == "ne"
    }) {
        return None;
    }
    let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();
    if cx.tcx.crate_name(trait_ref.def_id.krate).as_str() != "core"
        || cx.tcx.item_name(trait_ref.def_id).as_str() != "PartialEq"
        || trait_ref.self_ty() != trait_ref.args.type_at(1)
    {
        return None;
    }
    let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
        return None;
    };
    if !definition.is_struct() || !cx.tcx.generics_of(definition.did()).own_params.is_empty() {
        return None;
    }
    let body = cx.tcx.hir_body(body_id);
    let [left, right] = body.params else {
        return None;
    };
    let PatKind::Binding(_, left, _, None) = left.pat.kind else {
        return None;
    };
    let PatKind::Binding(_, right, _, None) = right.pat.kind else {
        return None;
    };
    let expression = DirectForwarding::single_body_expression(body.value)?;
    let mut fields = HashSet::new();
    collect_equal_fields(cx, expression, left, right, &mut fields)?;
    (!fields.is_empty()).then_some((definition.did().as_local()?, fields.len()))
}

fn collect_equal_fields(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    left: rustc_hir::HirId,
    right: rustc_hir::HirId,
    fields: &mut HashSet<String>,
) -> Option<()> {
    let ExprKind::Binary(operator, lhs, rhs) = expression.kind else {
        return None;
    };
    if operator.node == BinOpKind::And {
        collect_equal_fields(cx, lhs, left, right, fields)?;
        return collect_equal_fields(cx, rhs, left, right, fields);
    }
    if operator.node != BinOpKind::Eq {
        return None;
    }
    let (lhs_base, lhs_field) = field_access(lhs)?;
    let (rhs_base, rhs_field) = field_access(rhs)?;
    if lhs_field != rhs_field
        || !(DirectForwarding::is_binding(cx, lhs_base, left)
            && DirectForwarding::is_binding(cx, rhs_base, right)
            || DirectForwarding::is_binding(cx, lhs_base, right)
                && DirectForwarding::is_binding(cx, rhs_base, left))
        || !fields.insert(lhs_field)
    {
        return None;
    }
    Some(())
}

fn field_access<'hir>(expression: &'hir Expr<'hir>) -> Option<(&'hir Expr<'hir>, String)> {
    let ExprKind::Field(base, field) = expression.kind else {
        return None;
    };
    Some((base, field.name.to_string()))
}
