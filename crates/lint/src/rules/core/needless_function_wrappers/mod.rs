extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_abi::ExternAbi;
use rustc_errors::DiagDecorator;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Body, FnHeader, HirId, ImplItem, ImplItemKind, Item, ItemKind, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

// -----------------------------------------------------------------------------
// RedundantWrapper: Forwarding wrapper model
// -----------------------------------------------------------------------------

/// Function identity and signature facts needed during wrapper discovery.
struct RedundantWrapperIdentity {
    /// Local function definition being classified.
    def_id: LocalDefId,
    /// HIR node receiving any resulting diagnostic.
    hir_id: HirId,
    /// Wrapper name shown in diagnostics.
    name: Symbol,
    /// Wrapper identifier span used as the primary diagnostic location.
    name_span: Span,
    /// Function header used to preserve ABI, safety, and async semantics.
    header: FnHeader,
}

/// A function whose body only forwards its parameters to another local function.
struct RedundantWrapper {
    /// Wrapper HIR node used for diagnostic ownership.
    hir_id: HirId,
    /// Wrapper name shown in remediation guidance.
    name: Symbol,
    /// Identifier span used as the primary diagnostic site.
    name_span: Span,
    /// Local implementation to which every argument is forwarded.
    target: LocalDefId,
}

impl RedundantWrapper {
    /// Recognizes a safe wrapper that forwards every parameter unchanged to one local target.
    fn discover<'tcx>(
        cx: &LateContext<'tcx>,
        identity: &RedundantWrapperIdentity,
        body: &'tcx Body<'tcx>,
    ) -> Option<Self> {
        // Require an authored, safe Rust function with only nonsemantic attributes.
        if identity.header.abi != ExternAbi::Rust
            || identity.header.is_unsafe()
            || identity.name_span.from_expansion()
            || !DirectForwarding::has_only_nonsemantic_attributes(cx, identity.hir_id)
        {
            return None;
        }

        // Resolve plain parameter bindings and the function's one forwarding expression.
        let forwarding = DirectForwarding::expression(cx, identity.def_id, identity.header, body)?;

        // Require a distinct local call target with compatible async and receiver semantics.
        let call = DirectForwarding::call(cx, forwarding.typeck_owner, forwarding.forwarded)?;
        if !matches!(cx.tcx.def_kind(call.target), DefKind::Fn | DefKind::AssocFn) {
            return None;
        }
        let target = call.target.as_local()?;
        if target == identity.def_id
            || call.arguments.len() != forwarding.bindings.len()
            || (identity.header.is_async() && !Self::is_async_function(cx, target))
            || (call.is_method && !Self::shares_inherent_type(cx, identity.def_id, target))
        {
            return None;
        }

        // Compare the wrapper signature with the forwarded expression and target method.
        let typeck = cx.tcx.typeck(forwarding.typeck_owner);
        let signature = cx
            .tcx
            .fn_sig(identity.def_id)
            .instantiate_identity()
            .skip_binder();

        // Require the wrapper's arity and direct return type to match the forwarded call.
        if signature.inputs().len() != call.arguments.len()
            || (!identity.header.is_async()
                && signature.output() != typeck.expr_ty(forwarding.forwarded))
        {
            return None;
        }
        if call.is_method {
            let target_signature = cx
                .tcx
                .fn_sig(target)
                .instantiate(cx.tcx, typeck.node_args(forwarding.forwarded.hir_id))
                .skip_binder();
            if target_signature.inputs().len() != signature.inputs().len()
                || !Self::same_receiver_type(target_signature.inputs()[0], signature.inputs()[0])
            {
                return None;
            }
        }

        let bound_arguments = forwarding.bindings.iter().zip(call.arguments);
        let typed_arguments = bound_arguments.zip(signature.inputs());
        for (index, ((binding, argument), input)) in typed_arguments.enumerate() {
            // Require each forwarded argument to be its corresponding plain binding.
            // Compare the binding identity and adjusted argument type in parameter order.
            let has_incompatible_type =
                !(call.is_method && index == 0) && typeck.expr_ty_adjusted(argument) != *input;
            let has_matching_binding = DirectForwarding::is_binding(cx, argument, *binding);
            if has_matching_binding && !has_incompatible_type {
                continue;
            }
            return None;
        }

        // Retain the wrapper identity and its semantically equivalent target.
        Some(Self {
            hir_id: identity.hir_id,
            name: identity.name,
            name_span: identity.name_span,
            target,
        })
    }

    /// Returns whether a local free function or inherent method is asynchronous.
    fn is_async_function(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
        match cx.tcx.hir_node_by_def_id(def_id) {
            Node::Item(item) => {
                matches!(item.kind, ItemKind::Fn { sig, .. } if sig.header.is_async())
            }
            Node::ImplItem(item) => {
                matches!(item.kind, ImplItemKind::Fn(sig, _) if sig.header.is_async())
            }
            _ => false,
        }
    }

    /// Prevents receiver auto-deref from turning a meaningful adapter into a candidate while still
    /// allowing wrappers that call a method from another impl block for the same type.
    fn shares_inherent_type(cx: &LateContext<'_>, wrapper: LocalDefId, target: LocalDefId) -> bool {
        // Require both functions to belong to inherent implementation blocks.
        let wrapper_impl = cx.tcx.local_parent(wrapper);
        let target_impl = cx.tcx.local_parent(target);

        // Reject trait implementations before comparing nominal receiver types.
        if !matches!(
            cx.tcx.def_kind(wrapper_impl),
            DefKind::Impl { of_trait: false }
        ) || !matches!(
            cx.tcx.def_kind(target_impl),
            DefKind::Impl { of_trait: false }
        ) {
            return false;
        }

        // Compare the nominal self types after resolving each implementation.
        let wrapper_type = cx
            .tcx
            .type_of(wrapper_impl)
            .instantiate_identity()
            .ty_adt_def();

        // Resolve the target implementation's nominal self type independently.
        let target_type = cx
            .tcx
            .type_of(target_impl)
            .instantiate_identity()
            .ty_adt_def();
        wrapper_type == target_type
    }

    /// Compares receiver types while requiring reference mutability to match exactly.
    fn same_receiver_type<'tcx>(left: Ty<'tcx>, right: Ty<'tcx>) -> bool {
        match (left.kind(), right.kind()) {
            (ty::Ref(_, left, left_mutability), ty::Ref(_, right, right_mutability)) => {
                left == right && left_mutability == right_mutability
            }
            _ => left == right,
        }
    }
}

// -----------------------------------------------------------------------------
// Violation: Redundant forwarding diagnostic
// -----------------------------------------------------------------------------

/// Complete wrapper-removal diagnostic with its resolved target context.
struct Violation {
    /// Wrapper HIR node used for diagnostic ownership.
    hir_id: HirId,
    /// Wrapper name shown in remediation guidance.
    name: Symbol,
    /// Wrapper identifier span used as the primary diagnostic site.
    name_span: Span,
    /// Resolved forwarded implementation path.
    target_name: String,
    /// Forwarded implementation span shown as related evidence.
    target_span: Span,
}

impl Violation {
    /// Resolves the user-facing target facts required to remove one discovered wrapper.
    fn from_wrapper(cx: &LateContext<'_>, wrapper: &RedundantWrapper) -> Self {
        Self {
            hir_id: wrapper.hir_id,
            name: wrapper.name,
            name_span: wrapper.name_span,
            target_name: cx.tcx.def_path_str(wrapper.target.to_def_id()),
            target_span: cx.tcx.def_span(wrapper.target),
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "function `{}` is a redundant forwarding wrapper",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` adds an API and navigation layer without changing the behavior of `{}`",
            self.name, self.target_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "remove `{}` and update its callers to invoke `{}` directly; move its documentation or adjust the target's name and visibility if needed",
            self.name, self.target_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            NEEDLESS_FUNCTION_WRAPPERS,
            self.hir_id,
            self.name_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.name_span, "delete this useless indirection");
                diag.span_label(
                    self.target_span,
                    format!("forwarded implementation `{}`", self.target_name),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// NeedlessFunctionWrappers: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that detects semantically empty forwarding functions.
struct NeedlessFunctionWrappers;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub NEEDLESS_FUNCTION_WRAPPERS,
    Warn,
    "forbids redundant function forwarding wrappers",
    NeedlessFunctionWrappers
}

impl<'tcx> LateLintPass<'tcx> for NeedlessFunctionWrappers {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Require an authored free function body before extracting its syntax.
        if !matches!(item.kind, ItemKind::Fn { has_body: true, .. }) {
            return;
        }

        // Extract the signature and body from the validated function.
        let ItemKind::Fn { sig, body, .. } = item.kind else {
            return;
        };
        let Some(ident) = item.kind.ident() else {
            return;
        };

        // Analyze the authored body and emit only complete forwarding wrappers.
        let body = cx.tcx.hir_body(body);

        // Group the function identity and signature facts consumed by discovery.
        let identity = RedundantWrapperIdentity {
            def_id: item.owner_id.def_id,
            hir_id: item.hir_id(),
            name: ident.name,
            name_span: ident.span,
            header: sig.header,
        };

        // Discover one direct forwarding target without guessing through adapters.
        let Some(wrapper) = RedundantWrapper::discover(cx, &identity, body) else {
            return;
        };

        // Report the complete redundant wrapper after semantic validation.
        Violation::from_wrapper(cx, &wrapper).emit(cx);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        // Retain methods with authored bodies.
        let ImplItemKind::Fn(signature, body) = item.kind else {
            return;
        };

        // Resolve the enclosing implementation before classifying the method.
        let parent = cx.tcx.local_parent(item.owner_id.def_id);
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(parent) else {
            return;
        };

        // Only analyze inherent methods that are not trait implementations.
        let ItemKind::Impl(implementation) = parent.kind else {
            return;
        };

        // Trait methods intentionally adapt an external contract and are not redundant wrappers.
        if implementation.of_trait.is_some() {
            return;
        }

        // Group the method identity and signature facts consumed by discovery.
        let identity = RedundantWrapperIdentity {
            def_id: item.owner_id.def_id,
            hir_id: item.hir_id(),
            name: item.ident.name,
            name_span: item.ident.span,
            header: signature.header,
        };

        // Discover one direct forwarding target without guessing through adapters.
        let body = cx.tcx.hir_body(body);
        let Some(wrapper) = RedundantWrapper::discover(cx, &identity, body) else {
            return;
        };

        // Report the complete redundant wrapper after semantic validation.
        Violation::from_wrapper(cx, &wrapper).emit(cx);
    }
}
