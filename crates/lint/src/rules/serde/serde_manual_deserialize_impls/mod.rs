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
            "manual transparent deserializer for `{}` is derivable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the complete implementation decodes the sole field unchanged and immediately constructs the wrapper",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace this implementation with `#[derive(serde::Deserialize)]` and `#[serde(transparent)]`",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SERDE_MANUAL_DESERIALIZE_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this deserializer is exact transparent forwarding",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `exact_transparent_deserializer` step of the lint analysis.
fn exact_transparent_deserializer(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
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
    if method.sig.ident != "deserialize" {
        return false;
    }
    let [syn::FnArg::Typed(deserializer)] = method.sig.inputs.iter().collect::<Vec<_>>().as_slice()
    // Perform the next step of the analysis.
    else {
        return false;
    };
    let syn::Pat::Ident(deserializer) = deserializer.pat.as_ref() else {
        return false;
    };

    // Prepare the values used by this stage.
    let [syn::Stmt::Expr(syn::Expr::Call(ok), _)] = method.block.stmts.as_slice() else {
        return false;
    };
    if !matches!(ok.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Ok"))
        || ok.args.len() != 1
    {
        return false;
    }
    let Some(syn::Expr::Call(construction)) = ok.args.first() else {
        return false;
    };

    // Reject inputs that do not satisfy this stage.
    if !matches!(construction.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Self"))
        || construction.args.len() != 1
    {
        return false;
    }

    // Prepare the values used by this stage.
    let Some(syn::Expr::Try(decoded)) = construction.args.first() else {
        return false;
    };
    let syn::Expr::Call(decode) = decoded.expr.as_ref() else {
        return false;
    };

    // Perform the next step of the analysis.
    decode.args.len() == 1
        && matches!(decode.func.as_ref(), syn::Expr::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "deserialize"))
        && matches!(decode.args.first(), Some(syn::Expr::Path(path))
            if path.path.is_ident(&deserializer.ident))
}

/// Carries the `SerdeManualDeserializeImpls` state used by this analysis.
struct SerdeManualDeserializeImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_MANUAL_DESERIALIZE_IMPLS,
    Warn,
    "finds transparent Deserialize implementations reproducible by Serde derive",
    SerdeManualDeserializeImpls
}

impl LateLintPass<'_> for SerdeManualDeserializeImpls {
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
        if cx.tcx.item_name(trait_id).as_str() != "Deserialize"
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
            || !exact_transparent_deserializer(cx, item)
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
