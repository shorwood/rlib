extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, ImplItem, ImplItemKind, ItemKind, Mutability, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::{DefId, LocalDefId};

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Contract {
    derive: &'static str,
    forward: bool,
}

struct Family {
    span: Span,
    name: String,
    contracts: Vec<Contract>,
}

struct Violation(Family);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual forwarding interfaces for `{}` are derivable",
            self.0.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "these implementations only expose one field directly or delegate to that field's matching standard interface",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        let derives = self
            .0
            .contracts
            .iter()
            .map(|contract| format!("derive_more::{}", contract.derive))
            .collect::<Vec<_>>()
            .join(", ");
        let forwarding = self.0.contracts.iter().any(|contract| contract.forward);
        let qualifier = if forwarding {
            "; preserve pass-through targets with the corresponding `forward` attribute"
        } else {
            ""
        };
        Cow::Owned(format!(
            "replace this family with `#[derive({derives})]`{qualifier}"
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_MANUAL_FORWARDING_INTERFACES,
            self.0.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.0.span, "this type owns structural forwarding plumbing");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct DeriveMoreManualForwardingInterfaces {
    families: HashMap<LocalDefId, Family>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_FORWARDING_INTERFACES,
    Warn,
    "finds forwarding interfaces reproducible by derive_more",
    DeriveMoreManualForwardingInterfaces::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreManualForwardingInterfaces {
    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        let Some((definition, contract)) = exact_contract(cx, item) else {
            return;
        };
        let family = self.families.entry(definition).or_insert_with(|| Family {
            span: cx.tcx.def_span(definition),
            name: cx.tcx.item_name(definition).to_string(),
            contracts: Vec::new(),
        });
        if !family.contracts.contains(&contract) {
            family.contracts.push(contract);
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (_, mut family) in self.families.drain() {
            family.contracts.sort();
            Violation(family).emit(cx);
        }
    }
}

fn exact_contract(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<(LocalDefId, Contract)> {
    let ImplItemKind::Fn(signature, body_id) = item.kind else {
        return None;
    };
    if item.span.from_expansion() {
        return None;
    }
    let implementation = cx.tcx.local_parent(item.owner_id.def_id);
    let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
        return None;
    };
    let ItemKind::Impl(implementation_item) = parent.kind else {
        return None;
    };
    let trait_id = implementation_item.of_trait?.trait_ref.trait_def_id()?;
    let derive = supported_trait(cx, trait_id, item.ident.name.as_str())?;
    let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();
    let ty::Adt(wrapper, _) = trait_ref.self_ty().kind() else {
        return None;
    };
    let definition = wrapper.did().as_local()?;
    if !wrapper.is_struct() {
        return None;
    }
    let body = cx.tcx.hir_body(body_id);
    let forwarding =
        DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;
    let expected_bindings = if matches!(derive, "Index" | "IndexMut") {
        2
    } else {
        1
    };
    if forwarding.bindings.len() != expected_bindings {
        return None;
    }
    let self_binding = forwarding.bindings[0];
    if !matches!(derive, "Index" | "IndexMut")
        && let Some((field, mutability)) = direct_field_reference(forwarding.forwarded)
        && mutability == expected_mutability(derive)
        && DirectForwarding::is_binding(cx, field.0, self_binding)
    {
        let field_type = cx.tcx.typeck(forwarding.typeck_owner).expr_ty(field.1);
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();
        let ty::Ref(_, target, _) = output.kind() else {
            return None;
        };
        return Some((
            definition,
            Contract {
                derive,
                forward: field_type != *target,
            },
        ));
    }
    forwarded_contract(
        cx,
        forwarding.typeck_owner,
        forwarding.forwarded,
        &forwarding.bindings,
        trait_id,
        derive,
        definition,
    )
}

fn supported_trait(cx: &LateContext<'_>, trait_id: DefId, method: &str) -> Option<&'static str> {
    if cx.tcx.crate_name(trait_id.krate).as_str() != "core" {
        return None;
    }
    match (cx.tcx.item_name(trait_id).as_str(), method) {
        ("AsRef", "as_ref") => Some("AsRef"),
        ("AsMut", "as_mut") => Some("AsMut"),
        ("Deref", "deref") => Some("Deref"),
        ("DerefMut", "deref_mut") => Some("DerefMut"),
        ("Index", "index") => Some("Index"),
        ("IndexMut", "index_mut") => Some("IndexMut"),
        _ => None,
    }
}

fn expected_mutability(derive: &str) -> Mutability {
    if matches!(derive, "AsMut" | "DerefMut" | "IndexMut") {
        Mutability::Mut
    } else {
        Mutability::Not
    }
}

const fn direct_field_reference<'hir>(
    expression: &'hir Expr<'hir>,
) -> Option<((&'hir Expr<'hir>, &'hir Expr<'hir>), Mutability)> {
    let ExprKind::AddrOf(_, mutability, field_expression) = expression.kind else {
        return None;
    };
    let ExprKind::Field(base, _) = field_expression.kind else {
        return None;
    };
    Some(((base, field_expression), mutability))
}

fn forwarded_contract(
    cx: &LateContext<'_>,
    owner: LocalDefId,
    expression: &Expr<'_>,
    bindings: &[rustc_hir::HirId],
    trait_id: DefId,
    derive: &'static str,
    definition: LocalDefId,
) -> Option<(LocalDefId, Contract)> {
    let call = DirectForwarding::call(cx, owner, expression)?;
    if cx.tcx.trait_of_assoc(call.target) != Some(trait_id) {
        return None;
    }
    let first = call.arguments.first()?;
    if !field_argument(cx, first, bindings[0], expected_mutability(derive)) {
        return None;
    }
    if matches!(derive, "Index" | "IndexMut")
        && (call.arguments.len() != 2
            || !DirectForwarding::is_binding(cx, call.arguments[1], bindings[1]))
    {
        return None;
    }
    Some((
        definition,
        Contract {
            derive,
            forward: true,
        },
    ))
}

fn field_argument(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    binding: rustc_hir::HirId,
    mutability: Mutability,
) -> bool {
    let expression = match expression.kind {
        ExprKind::AddrOf(_, actual, inner) if actual == mutability => inner,
        _ => expression,
    };
    let ExprKind::Field(base, _) = expression.kind else {
        return false;
    };
    DirectForwarding::is_binding(cx, base, binding)
}
