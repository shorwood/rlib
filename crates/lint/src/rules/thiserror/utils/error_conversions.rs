extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::{ExprKind, ImplItem, ImplItemKind, ItemKind, Node, StructTailExpr};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{ThiserrorAttributes, is_transparent_error};
use crate::utils::direct_forwarding::DirectForwarding;
use crate::utils::source_provenance::AuthoredItemSource;

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
        // Conversion candidates must be authored function implementations.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // Generated methods and receiver methods cannot be the static conversion entry point.
        if item.span.from_expansion() || signature.decl.implicit_self.has_implicit_self() {
            return None;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // The method must belong to a source-level implementation item.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return None;
        };

        // Nonimplementation parents cannot define a conversion contract.
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return None;
        };

        let trait_id = implementation_item
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;

        // Exclude methods outside the canonical core conversion implementation.
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || cx.tcx.item_name(trait_id).as_str() != "From"
            || item.ident.name.as_str() != "from"
        {
            return None;
        }
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();
        let source = trait_ref.args.type_at(1);
        let target = trait_ref.self_ty();

        // Only algebraic data types can provide enum variants for wrapping.
        let ty::Adt(definition, arguments) = target.kind() else {
            return None;
        };

        // Struct conversions cannot be replaced by a derived enum source variant.
        if !definition.is_enum() {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);

        let forwarding =
            DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;

        // The conversion must forward exactly its single source argument.
        let [binding] = forwarding.bindings.as_slice() else {
            return None;
        };
        let (target, argument) = if let Some(call) =
            DirectForwarding::call(cx, forwarding.typeck_owner, forwarding.forwarded)
        {
            // Constructor calls must receive only the forwarded source value.
            let [argument] = call.arguments.as_slice() else {
                return None;
            };
            (call.target, *argument)
        } else {
            // Struct syntax must construct one field without an update tail.
            let ExprKind::Struct(path, [field], StructTailExpr::None) = forwarding.forwarded.kind
            else {
                return None;
            };
            (
                cx.qpath_res(path, forwarding.forwarded.hir_id)
                    .opt_def_id()?,
                field.expr,
            )
        };

        // Reject conversions that transform rather than directly forward the source.
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
                && (target == variant.def_id || cx.tcx.opt_parent(target) == Some(variant.def_id))
        })?;

        // Recover the authored variant field so derive attributes can be compared precisely.
        let Node::Item(enum_item) = cx.tcx.hir_node_by_def_id(definition.did().as_local()?) else {
            return None;
        };
        let source_text = AuthoredItemSource::for_item(cx, enum_item)?;

        // Unparseable authored enum text cannot provide reliable derive attributes.
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source_text) else {
            return None;
        };

        // Match the resolved compiler variant to its authored field declaration.
        let authored_variant = enumeration
            .variants
            .iter()
            .find(|candidate| candidate.ident == variant.name.as_str())?;
        let authored_fields = authored_variant.fields.iter().collect::<Vec<_>>();

        // Derivation is equivalent only for a unique wrapped source field.
        let [authored_field] = authored_fields.as_slice() else {
            return None;
        };

        // Require the wrapped field to participate in the error source chain.
        let is_source = authored_field
            .ident
            .as_ref()
            .is_some_and(|name| name == "source")
            || ThiserrorAttributes::from_attributes(&authored_field.attrs).is_source
            || is_transparent_error(&authored_variant.attrs);

        // Fields outside the source chain do not represent error conversion policy.
        if !is_source {
            return None;
        }

        Some(Self {
            definition: definition.did().as_local()?,
            span: parent.span,
            error: cx.tcx.item_name(definition.did()).to_string(),
            variant: variant.name.to_string(),
        })
    }
}
