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

use super::contracts::authored_item_source;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    name: String,
    derive: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual `{}` implementation for `{}` is derivable",
            self.derive, self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the implementation maps the sole field and delegates directly to the same aggregation without changing identity or failure behavior",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "replace this implementation with `#[derive(derive_more::{})]`",
            self.derive
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_MANUAL_AGGREGATION_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this aggregation is exact newtype forwarding");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct DeriveMoreManualAggregationImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_AGGREGATION_IMPLS,
    Warn,
    "finds Sum and Product implementations reproducible by derive_more",
    DeriveMoreManualAggregationImpls
}

impl LateLintPass<'_> for DeriveMoreManualAggregationImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        if item.span.from_expansion() || !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return;
        }
        let Some(trait_id) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };
        let derive_name = cx.tcx.item_name(trait_id);
        let derive = derive_name.as_str();
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || !matches!(derive, "Sum" | "Product")
        {
            return;
        }
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return;
        };
        if !definition.is_struct()
            || definition.non_enum_variant().fields.len() != 1
            || !cx.tcx.generics_of(definition.did()).own_params.is_empty()
            || !exact_aggregation_source(cx, item, derive)
        {
            return;
        }
        Violation {
            owner: item.hir_id(),
            span: item.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
            derive: derive.to_owned(),
        }
        .emit(cx);
    }
}

fn exact_aggregation_source(cx: &LateContext<'_>, item: &Item<'_>, derive: &str) -> bool {
    let Some(source) = authored_item_source(cx, item) else {
        return false;
    };
    let Ok(implementation) = syn::parse_str::<syn::ItemImpl>(&source) else {
        return false;
    };
    let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
        return false;
    };
    let expected_method = derive.to_ascii_lowercase();
    if method.sig.ident != expected_method {
        return false;
    }
    let [syn::FnArg::Typed(parameter)] = method.sig.inputs.iter().collect::<Vec<_>>().as_slice()
    else {
        return false;
    };
    let syn::Pat::Ident(parameter) = parameter.pat.as_ref() else {
        return false;
    };
    let [syn::Stmt::Expr(syn::Expr::Call(construction), _)] = method.block.stmts.as_slice() else {
        return false;
    };
    if !matches!(construction.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Self"))
        || construction.args.len() != 1
    {
        return false;
    }
    let Some(syn::Expr::MethodCall(aggregation)) = construction.args.first() else {
        return false;
    };
    if aggregation.method != expected_method || !aggregation.args.is_empty() {
        return false;
    }
    let syn::Expr::MethodCall(map) = aggregation.receiver.as_ref() else {
        return false;
    };
    if map.method != "map"
        || !matches!(map.receiver.as_ref(), syn::Expr::Path(path) if path.path.is_ident(&parameter.ident))
        || map.args.len() != 1
    {
        return false;
    }
    let Some(syn::Expr::Closure(projection)) = map.args.first() else {
        return false;
    };
    let [syn::Pat::Ident(value)] = projection.inputs.iter().collect::<Vec<_>>().as_slice() else {
        return false;
    };
    matches!(
        projection.body.as_ref(),
        syn::Expr::Field(field)
            if matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident(&value.ident))
                && matches!(&field.member, syn::Member::Unnamed(index) if index.index == 0)
    )
}
