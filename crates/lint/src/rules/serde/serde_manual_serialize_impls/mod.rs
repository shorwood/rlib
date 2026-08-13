extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual transparent serializer for `{}` is derivable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the complete implementation forwards the sole field unchanged to the provided serializer",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace this implementation with `#[derive(serde::Serialize)]` and `#[serde(transparent)]`",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SERDE_MANUAL_SERIALIZE_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this serializer is exact transparent forwarding");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `exact_transparent_serializer` step of the lint analysis.
fn exact_transparent_serializer(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
    // Prepare the values used by this stage.
    let Some(source) = AuthoredItemSource::for_item(cx, item) else {
        return false;
    };
    let Ok(implementation) = syn::parse_str::<syn::ItemImpl>(&source) else {
        return false;
    };

    // Prepare the values used by this stage.
    let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
        return false;
    };
    if method.sig.ident != "serialize" {
        return false;
    }

    // Prepare the values used by this stage.
    let [_, syn::FnArg::Typed(serializer)] =
        method.sig.inputs.iter().collect::<Vec<_>>().as_slice()
    else {
        return false;
    };

    // Prepare the values used by this stage.
    let syn::Pat::Ident(serializer) = serializer.pat.as_ref() else {
        return false;
    };
    let [syn::Stmt::Expr(syn::Expr::MethodCall(call), _)] = method.block.stmts.as_slice() else {
        return false;
    };

    // Perform the next step of the analysis.
    call.method == "serialize"
        && call.args.len() == 1
        && matches!(call.receiver.as_ref(), syn::Expr::Field(field)
            if matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("self"))
                && matches!(&field.member, syn::Member::Unnamed(index) if index.index == 0))
        && matches!(call.args.first(), Some(syn::Expr::Path(path)) if path.path.is_ident(&serializer.ident))
}

/// Carries the `SerdeManualSerializeImpls` state used by this analysis.
struct SerdeManualSerializeImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_MANUAL_SERIALIZE_IMPLS,
    Warn,
    "finds transparent Serialize implementations reproducible by Serde derive",
    SerdeManualSerializeImpls
}

impl LateLintPass<'_> for SerdeManualSerializeImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Prepare the values used by this stage.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        if item.span.from_expansion() || !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return;
        }

        // Prepare the values used by this stage.
        let Some(trait_id) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };

        // Reject inputs that do not satisfy this stage.
        if cx.tcx.item_name(trait_id).as_str() != "Serialize"
            || !matches!(
                cx.tcx.crate_name(trait_id.krate).as_str(),
                "serde" | "serde_core"
            )
        // Perform the next step of the analysis.
        {
            return;
        }
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();

        // Prepare the values used by this stage.
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return;
        };
        if !definition.is_struct()
            || definition.non_enum_variant().fields.len() != 1
            || !cx.tcx.generics_of(definition.did()).own_params.is_empty()
            || !exact_transparent_serializer(cx, item)
        {
            return;
        }

        // Perform the next step of the analysis.
        Violation {
            owner: item.hir_id(),
            span: item.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
        }
        .emit(cx);
    }
}
