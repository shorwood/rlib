extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::{ExprKind, ImplItem, ImplItemKind, ItemKind, Mutability, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Classifies `Receiver` cases used by this analysis.
enum Receiver {
    /// Represents the `Owned` case.
    Owned,
    /// Represents the `Ref` case.
    Ref,
    /// Represents the `RefMut` case.
    RefMut,
}

impl Receiver {
    /// Performs the `attribute` operation for this value.
    const fn attribute(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::Ref => "ref",
            Self::RefMut => "ref_mut",
        }
    }
}

/// Carries the `Family` state used by this analysis.
struct Family {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `receivers` value used by this analysis.
    receivers: Vec<Receiver>,
}

/// Stores the `item` value used by this analysis.
struct Violation(
    /// Iterator family that triggered the violation.
    Family,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual transparent iteration for `{}` is derivable",
            self.0.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("each implementation delegates unchanged to the wrapper's sole field")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        // Prepare the values used by this stage.
        let receivers = self
            .0
            .receivers
            .iter()
            .map(|receiver| receiver.attribute())
            .collect::<Vec<_>>()
            .join(", ");

        // Perform the next step of the analysis.
        Cow::Owned(format!(
            "replace this family with `#[derive(derive_more::IntoIterator)]` and `#[into_iterator({receivers})]`"
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_MANUAL_INTO_ITERATOR_IMPLS,
            self.0.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.0.span, "this wrapper owns direct field iteration");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Identifies one transparent iteration implementation.
struct ExactDelegation {
    /// Local wrapper type receiving the implementation.
    definition: LocalDefId,
    /// Form of receiver forwarded to the wrapped field.
    receiver: Receiver,
}

impl ExactDelegation {
    /// Recognizes one transparent iteration implementation.
    fn analyze(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };
        if item.ident.name.as_str() != "into_iter" || item.span.from_expansion() {
            return None;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // Prepare the values used by this stage.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return None;
        };
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return None;
        };
        let trait_id = implementation_item.of_trait?.trait_ref.trait_def_id()?;

        // Reject inputs that do not satisfy this stage.
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || cx.tcx.item_name(trait_id).as_str() != "IntoIterator"
        {
            return None;
        }
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();

        // Prepare the values used by this stage.
        let (wrapper, receiver) = match trait_ref.self_ty().kind() {
            ty::Adt(definition, _) => (*definition, Receiver::Owned),
            ty::Ref(_, inner, Mutability::Not) => (inner.ty_adt_def()?, Receiver::Ref),
            ty::Ref(_, inner, Mutability::Mut) => (inner.ty_adt_def()?, Receiver::RefMut),
            _ => return None,
        };
        let definition = wrapper.did().as_local()?;

        // Reject inputs that do not satisfy this stage.
        if !wrapper.is_struct() || wrapper.non_enum_variant().fields.len() != 1 {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);
        let forwarding =
            DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;

        // Prepare the values used by this stage.
        let [binding] = forwarding.bindings.as_slice() else {
            return None;
        };
        let call = DirectForwarding::call(cx, forwarding.typeck_owner, forwarding.forwarded)?;
        let called_trait = cx.tcx.trait_of_assoc(call.target)?;

        // Reject inputs that do not satisfy this stage.
        if called_trait != trait_id || cx.tcx.item_name(call.target).as_str() != "into_iter" {
            return None;
        }
        let [argument] = call.arguments.as_slice() else {
            return None;
        };

        // Classify the current analyze_candidate.
        match (receiver, argument.kind) {
            (Receiver::Owned, ExprKind::Field(base, _))
                if DirectForwarding::is_binding(cx, base, *binding) => {}
            (Receiver::Ref, ExprKind::AddrOf(_, Mutability::Not, inner))
            | (Receiver::RefMut, ExprKind::AddrOf(_, Mutability::Mut, inner)) => {
                let ExprKind::Field(base, _) = inner.kind else {
                    return None;
                };
                if !DirectForwarding::is_binding(cx, base, *binding) {
                    return None;
                }
            }
            _ => return None,
        }

        // Return the completed analysis result.
        Some(Self {
            definition,
            receiver,
        })
    }
}

#[derive(Default)]
/// Carries the `DeriveMoreManualIntoIteratorImpls` state used by this analysis.
struct DeriveMoreManualIntoIteratorImpls {
    /// Stores the `families` value used by this analysis.
    families: HashMap<LocalDefId, Family>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_INTO_ITERATOR_IMPLS,
    Warn,
    "finds transparent IntoIterator implementations reproducible by derive_more",
    DeriveMoreManualIntoIteratorImpls::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreManualIntoIteratorImpls {
    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        let Some(ExactDelegation {
            definition,
            receiver,
        }) = ExactDelegation::analyze(cx, item)
        else {
            return;
        };
        let family = self.families.entry(definition).or_insert_with(|| Family {
            span: cx.tcx.def_span(definition),
            name: cx.tcx.item_name(definition).to_string(),
            receivers: Vec::new(),
        });
        if family.receivers.contains(&receiver) {
            return;
        }
        family.receivers.push(receiver);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (_, mut family) in self.families.drain() {
            family.receivers.sort();
            Violation(family).emit(cx);
        }
    }
}
