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

/// Receiver forms supported by `derive_more`'s `IntoIterator` derive.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Receiver {
    /// Consumes the wrapper and yields owned items.
    Owned,
    /// Iterates through a shared reference to the wrapper.
    Ref,
    /// Iterates through a mutable reference to the wrapper.
    RefMut,
}

impl Receiver {
    /// Returns `derive_more`'s attribute spelling for this receiver form.
    const fn attribute(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::Ref => "ref",
            Self::RefMut => "ref_mut",
        }
    }
}

// -----------------------------------------------------------------------------
// Violation: Derivable iterator implementation family
// -----------------------------------------------------------------------------

/// Owned and borrowed iteration implementations for one transparent wrapper.
struct Family {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
    /// Owned and borrowed receiver forms implemented by the wrapper.
    receivers: Vec<Receiver>,
}

/// Complete iterator family proven replaceable by `derive_more`.
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
        let receivers = self
            .0
            .receivers
            .iter()
            .map(|receiver| receiver.attribute())
            .collect::<Vec<_>>()
            .join(", ");

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
        // Only function implementation items can define into-iterator delegation.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // Only authored into_iter methods can match the generated contract.
        if item.ident.name.as_str() != "into_iter" || item.span.from_expansion() {
            return None;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // Methods without an item-level parent cannot establish implementation ownership.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return None;
        };

        // Only implementation parents can carry the IntoIterator trait reference.
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return None;
        };

        // Attributes may add behavior the derive replacement cannot preserve.
        if !cx.tcx.hir_attrs(parent.hir_id()).is_empty()
            || !cx.tcx.hir_attrs(item.hir_id()).is_empty()
        {
            return None;
        }
        let trait_id = implementation_item.of_trait?.trait_ref.trait_def_id()?;

        // Only the core IntoIterator trait has the derive_more replacement contract.
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || cx.tcx.item_name(trait_id).as_str() != "IntoIterator"
        {
            return None;
        }
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();

        let (wrapper, receiver) = match trait_ref.self_ty().kind() {
            ty::Adt(definition, _) => (*definition, Receiver::Owned),
            ty::Ref(_, inner, Mutability::Not) => (inner.ty_adt_def()?, Receiver::Ref),
            ty::Ref(_, inner, Mutability::Mut) => (inner.ty_adt_def()?, Receiver::RefMut),
            // Other receiver forms are not supported by the derive contract.
            _ => return None,
        };
        let definition = wrapper.did().as_local()?;

        // Only one-field structs can transparently delegate iteration to their field.
        if !wrapper.is_struct() || wrapper.non_enum_variant().fields.len() != 1 {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);
        let forwarding =
            DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;

        // Transparent delegation must forward exactly the method receiver.
        let [binding] = forwarding.bindings.as_slice() else {
            return None;
        };
        let call = DirectForwarding::call(cx, forwarding.typeck_owner, forwarding.forwarded)?;
        let called_trait = cx.tcx.trait_of_assoc(call.target)?;

        // The forwarded call must invoke the same IntoIterator contract unchanged.
        if called_trait != trait_id || cx.tcx.item_name(call.target).as_str() != "into_iter" {
            return None;
        }

        // IntoIterator delegation must pass exactly one receiver argument.
        let [argument] = call.arguments.as_slice() else {
            return None;
        };

        match (receiver, argument.kind) {
            (Receiver::Owned, ExprKind::Field(base, _))
                if DirectForwarding::is_binding(cx, base, *binding) => {}
            (Receiver::Ref, ExprKind::AddrOf(_, Mutability::Not, inner))
            | (Receiver::RefMut, ExprKind::AddrOf(_, Mutability::Mut, inner)) => {
                // Borrowed delegation must select a field behind the matching reference.
                let ExprKind::Field(base, _) = inner.kind else {
                    return None;
                };

                // The selected field must remain rooted in the original receiver.
                if !DirectForwarding::is_binding(cx, base, *binding) {
                    return None;
                }
            }
            // Any other argument shape adds behavior beyond transparent field delegation.
            _ => return None,
        }

        Some(Self {
            definition,
            receiver,
        })
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreManualIntoIteratorImpls: Declarative iteration policy
// -----------------------------------------------------------------------------

/// Groups transparent `IntoIterator` implementations by their wrapper type.
#[derive(Default)]
struct DeriveMoreManualIntoIteratorImpls {
    /// Iterator families accumulated until every receiver form is known.
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
        // Nontransparent implementations do not contribute to a derivable receiver family.
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

        // Repeated receiver forms add no new family evidence.
        if family.receivers.contains(&receiver) {
            return;
        }
        family.receivers.push(receiver);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut families = self
            .families
            .drain()
            .map(|(_, family)| family)
            .collect::<Vec<_>>();
        families.sort_by_key(|family| family.span.lo());
        for mut family in families {
            family.receivers.sort();
            Violation(family).emit(cx);
        }
    }
}
