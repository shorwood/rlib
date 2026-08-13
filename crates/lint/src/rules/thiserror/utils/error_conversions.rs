extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::{ImplItem, ImplItemKind, ItemKind, Node};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::direct_forwarding::DirectForwarding;

/// Exact manual conversion that wraps one error in one enum variant.
pub struct Candidate {
    /// Target error enum definition.
    pub(crate) definition: LocalDefId,
    /// Manual implementation receiving a diagnostic.
    pub(crate) span: Span,
    /// Target error enum name.
    pub error: String,
    /// Single-field variant receiving the source value.
    pub variant: String,
}

impl Candidate {
    /// Recognizes an exact `From<T>` implementation that constructs one variant.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };
        if item.span.from_expansion() || signature.decl.implicit_self.has_implicit_self() {
            return None;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return None;
        };
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return None;
        };

        let trait_id = implementation_item
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || cx.tcx.item_name(trait_id).as_str() != "From"
            || item.ident.name.as_str() != "from"
        {
            return None;
        }
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();
        let source = trait_ref.args.type_at(1);
        let target = trait_ref.self_ty();

        let ty::Adt(definition, arguments) = target.kind() else {
            return None;
        };
        if !definition.is_enum() {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);

        let forwarding =
            DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;
        let [binding] = forwarding.bindings.as_slice() else {
            return None;
        };
        let call = DirectForwarding::call(cx, forwarding.typeck_owner, forwarding.forwarded)?;

        let [argument] = call.arguments.as_slice() else {
            return None;
        };
        if !DirectForwarding::is_binding(cx, argument, *binding) {
            return None;
        }

        // Require the called constructor to be the unique matching single-field variant.
        let variant = definition.variants().iter().find(|variant| {
            variant.fields.len() == 1
                && variant
                    .fields
                    .iter()
                    .next()
                    .is_some_and(|field| field.ty(cx.tcx, arguments) == source)
                && (call.target == variant.def_id
                    || cx.tcx.opt_parent(call.target) == Some(variant.def_id))
        })?;

        Some(Self {
            definition: definition.did().as_local()?,
            span: parent.span,
            error: cx.tcx.item_name(definition.did()).to_string(),
            variant: variant.name.to_string(),
        })
    }
}
