extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::source_provenance::authored_item_source;

#[derive(Clone)]
pub(crate) struct ManualErrorCandidate {
    pub(crate) span: Span,
    pub(crate) name: String,
    pub(crate) message: String,
    pub(crate) source_field: Option<String>,
}

#[derive(Default)]
pub(crate) struct ManualErrorCatalog {
    displays: HashMap<LocalDefId, String>,
    errors: HashMap<LocalDefId, Option<String>>,
    types: HashMap<LocalDefId, (Span, String)>,
}

impl ManualErrorCatalog {
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            return;
        }
        match item.kind {
            ItemKind::Struct(identifier, ..) => {
                self.types.insert(
                    item.owner_id.def_id,
                    (item.span, identifier.name.to_string()),
                );
            }
            ItemKind::Impl(implementation) => {
                let Some((definition, trait_name)) = impl_contract(cx, item, &implementation)
                else {
                    return;
                };
                if trait_name == "Display" {
                    if let Some(message) = display_message(cx, item) {
                        self.displays.insert(definition, message);
                    }
                } else if trait_name == "Error" {
                    if let Some(source_field) = error_source(cx, item) {
                        self.errors.insert(definition, source_field);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn candidates(&self) -> Vec<ManualErrorCandidate> {
        let mut candidates = self
            .errors
            .iter()
            .filter_map(|(definition, source_field)| {
                let message = self.displays.get(definition)?;
                let (span, name) = self.types.get(definition)?;
                Some(ManualErrorCandidate {
                    span: *span,
                    name: name.clone(),
                    message: message.clone(),
                    source_field: source_field.clone(),
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| candidate.span.lo());
        candidates
    }

    #[cfg(feature = "derive_more")]
    pub(crate) fn contains(&self, definition: LocalDefId) -> bool {
        self.displays.contains_key(&definition) && self.errors.contains_key(&definition)
    }
}

fn impl_contract(
    cx: &LateContext<'_>,
    item: &Item<'_>,
    implementation: &rustc_hir::Impl<'_>,
) -> Option<(LocalDefId, &'static str)> {
    if !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
        return None;
    }
    let trait_id = implementation
        .of_trait
        .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;
    if !matches!(cx.tcx.crate_name(trait_id.krate).as_str(), "core" | "std") {
        return None;
    }
    let trait_name = match cx.tcx.item_name(trait_id).as_str() {
        "Display" => "Display",
        "Error" => "Error",
        _ => return None,
    };
    let trait_ref = cx
        .tcx
        .impl_trait_ref(item.owner_id.def_id)
        .instantiate_identity();
    let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
        return None;
    };
    if !definition.is_struct() || !cx.tcx.generics_of(definition.did()).own_params.is_empty() {
        return None;
    }
    Some((definition.did().as_local()?, trait_name))
}

fn display_message(cx: &LateContext<'_>, item: &Item<'_>) -> Option<String> {
    let source = authored_item_source(cx, item)?;
    let implementation = syn::parse_str::<syn::ItemImpl>(&source).ok()?;
    let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
        return None;
    };
    if method.sig.ident != "fmt" {
        return None;
    }
    let mut inputs = method.sig.inputs.iter();
    if !matches!(inputs.next(), Some(syn::FnArg::Receiver(_))) {
        return None;
    }
    let Some(syn::FnArg::Typed(formatter)) = inputs.next() else {
        return None;
    };
    if inputs.next().is_some() {
        return None;
    }
    let syn::Pat::Ident(formatter) = formatter.pat.as_ref() else {
        return None;
    };
    let [syn::Stmt::Expr(syn::Expr::MethodCall(call), _)] = method.block.stmts.as_slice() else {
        return None;
    };
    if call.method != "write_str" || call.turbofish.is_some() || call.args.len() != 1 {
        return None;
    }
    if !matches!(call.receiver.as_ref(), syn::Expr::Path(path) if path.path.is_ident(&formatter.ident))
    {
        return None;
    }
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(message),
        ..
    }) = call.args.first()?
    else {
        return None;
    };
    let message = message.value();
    (!message.contains('{') && !message.contains('}')).then_some(message)
}

/// Returns `Some(None)` for an empty implementation and the direct source field otherwise.
fn error_source(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Option<String>> {
    let source = authored_item_source(cx, item)?;
    let implementation = syn::parse_str::<syn::ItemImpl>(&source).ok()?;
    match implementation.items.as_slice() {
        [] => Some(None),
        [syn::ImplItem::Fn(method)] => conventional_source(method).map(Some),
        _ => None,
    }
}

fn conventional_source(method: &syn::ImplItemFn) -> Option<String> {
    if method.sig.ident != "source" {
        return None;
    }
    let [syn::Stmt::Expr(syn::Expr::Call(call), _)] = method.block.stmts.as_slice() else {
        return None;
    };
    if !matches!(call.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Some")) {
        return None;
    }
    if call.args.len() != 1 {
        return None;
    }
    let argument = call.args.first()?;
    let syn::Expr::Reference(reference) = argument else {
        return None;
    };
    let syn::Expr::Field(field) = reference.expr.as_ref() else {
        return None;
    };
    if !matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("self")) {
        return None;
    }
    match &field.member {
        syn::Member::Named(name) => Some(name.to_string()),
        syn::Member::Unnamed(_) => None,
    }
}
