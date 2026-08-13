extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, ImplItemKind, ItemKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual transparent parser for `{}` is derivable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the implementation forwards the complete input to the sole field's parser and maps its unchanged error directly into the wrapper",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace this implementation with `#[derive(derive_more::FromStr)]` on the newtype",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_MANUAL_FROM_STR_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this parser is exact newtype forwarding");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct DeriveMoreManualFromStrImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_FROM_STR_IMPLS,
    Warn,
    "finds transparent FromStr implementations reproducible by derive_more",
    DeriveMoreManualFromStrImpls
}

impl LateLintPass<'_> for DeriveMoreManualFromStrImpls {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let ImplItemKind::Fn(_, _) = item.kind else {
            return;
        };
        if item.ident.name.as_str() != "from_str"
            || item.span.from_expansion()
            || !cx.tcx.hir_attrs(item.hir_id()).is_empty()
        {
            return;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return;
        };
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return;
        };
        let Some(trait_id) = implementation_item
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || cx.tcx.item_name(trait_id).as_str() != "FromStr"
        {
            return;
        }
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return;
        };
        if !definition.is_struct()
            || definition.non_enum_variant().fields.len() != 1
            || !cx.tcx.generics_of(definition.did()).own_params.is_empty()
            || !exact_forwarding_source(cx, item)
        {
            return;
        }
        Violation {
            owner: item.hir_id(),
            span: parent.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
        }
        .emit(cx);
    }
}

fn exact_forwarding_source(cx: &LateContext<'_>, item: &ImplItem<'_>) -> bool {
    let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
        return false;
    };
    let Ok(method) = syn::parse_str::<syn::ImplItemFn>(&source) else {
        return false;
    };
    let [syn::FnArg::Typed(parameter)] = method.sig.inputs.iter().collect::<Vec<_>>().as_slice()
    else {
        return false;
    };
    let syn::Pat::Ident(parameter) = parameter.pat.as_ref() else {
        return false;
    };
    let [syn::Stmt::Expr(expression, _)] = method.block.stmts.as_slice() else {
        return false;
    };
    let syn::Expr::MethodCall(map) = expression else {
        return false;
    };
    if map.method != "map" || map.args.len() != 1 {
        return false;
    }
    let Some(syn::Expr::Path(constructor)) = map.args.first() else {
        return false;
    };
    if !constructor.path.is_ident("Self") {
        return false;
    }
    exact_parse_receiver(map.receiver.as_ref(), &parameter.ident)
}

fn exact_parse_receiver(expression: &syn::Expr, parameter: &syn::Ident) -> bool {
    match expression {
        syn::Expr::MethodCall(parse) => {
            parse.method == "parse"
                && parse.args.is_empty()
                && matches!(parse.receiver.as_ref(), syn::Expr::Path(value) if value.path.is_ident(parameter))
        }
        syn::Expr::Call(parse) => {
            parse.args.len() == 1
                && matches!(parse.func.as_ref(), syn::Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "from_str"))
                && matches!(parse.args.first(), Some(syn::Expr::Path(value)) if value.path.is_ident(parameter))
        }
        _ => false,
    }
}
