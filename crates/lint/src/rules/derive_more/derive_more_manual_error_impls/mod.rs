extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::mem;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;

#[cfg(feature = "thiserror")]
use crate::rules::framework::config::{DeriveResolutionConfig, ErrorImplementationProvider};
#[cfg(feature = "thiserror")]
use crate::rules::thiserror::manual_error::ManualErrorCatalog;
#[cfg(feature = "thiserror")]
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Candidate {
    definition: LocalDefId,
    owner: HirId,
    span: Span,
    name: String,
}

struct Violation {
    owner: HirId,
    span: Span,
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual error implementation for `{}` is derivable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the implementation adds no behavior beyond derive_more's conventional error source handling",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace this implementation with `#[derive(derive_more::Error)]` and retain authored code only for custom error behavior",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_MANUAL_ERROR_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this complete error implementation is derivable");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct DeriveMoreManualErrorImpls {
    candidates: Vec<Candidate>,
    #[cfg(feature = "thiserror")]
    config: DeriveResolutionConfig,
    #[cfg(feature = "thiserror")]
    overlaps: ManualErrorCatalog,
}

impl DeriveMoreManualErrorImpls {
    fn new() -> Self {
        Self {
            candidates: Vec::new(),
            #[cfg(feature = "thiserror")]
            config: LibraryConfig::load().derive_resolution,
            #[cfg(feature = "thiserror")]
            overlaps: ManualErrorCatalog::default(),
        }
    }

    fn selected(&self, definition: LocalDefId) -> bool {
        #[cfg(feature = "thiserror")]
        if self.overlaps.contains(definition) {
            return self.config.error_implementation()
                == Some(ErrorImplementationProvider::DeriveMoreError);
        }
        let _ = definition;
        true
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_ERROR_IMPLS,
    Warn,
    "finds Error implementations reproducible by derive_more",
    DeriveMoreManualErrorImpls::new()
}

impl LateLintPass<'_> for DeriveMoreManualErrorImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        #[cfg(feature = "thiserror")]
        self.overlaps.check_item(cx, item);
        let Some(candidate) = candidate(cx, item) else {
            return;
        };
        self.candidates.push(candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in mem::take(&mut self.candidates) {
            if !self.selected(candidate.definition) {
                continue;
            }
            Violation {
                owner: candidate.owner,
                span: candidate.span,
                name: candidate.name,
            }
            .emit(cx);
        }
    }
}

fn candidate(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Candidate> {
    let ItemKind::Impl(implementation) = item.kind else {
        return None;
    };
    if item.span.from_expansion() || !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
        return None;
    }
    let trait_id = implementation
        .of_trait
        .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;
    if cx.tcx.item_name(trait_id).as_str() != "Error"
        || !matches!(cx.tcx.crate_name(trait_id.krate).as_str(), "core" | "std")
    {
        return None;
    }
    let trait_ref = cx
        .tcx
        .impl_trait_ref(item.owner_id.def_id)
        .instantiate_identity();
    let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
        return None;
    };
    if !definition.is_struct()
        || definition.did().as_local().is_none()
        || !cx.tcx.generics_of(definition.did()).own_params.is_empty()
        || !is_derivable_error_impl(cx, item)
    {
        return None;
    }
    Some(Candidate {
        definition: definition.did().as_local()?,
        owner: item.hir_id(),
        span: item.span,
        name: cx.tcx.item_name(definition.did()).to_string(),
    })
}

fn is_derivable_error_impl(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
    let Some(source) = authored_item_source(cx, item) else {
        return false;
    };
    let Ok(implementation) = syn::parse_str::<syn::ItemImpl>(&source) else {
        return false;
    };
    match implementation.items.as_slice() {
        [] => true,
        [syn::ImplItem::Fn(method)] => conventional_source(method),
        _ => false,
    }
}

fn conventional_source(method: &syn::ImplItemFn) -> bool {
    if method.sig.ident != "source" {
        return false;
    }
    let [syn::Stmt::Expr(syn::Expr::Call(call), _)] = method.block.stmts.as_slice() else {
        return false;
    };
    if !matches!(call.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Some")) {
        return false;
    }
    if call.args.len() != 1 {
        return false;
    }
    let Some(argument) = call.args.first() else {
        return false;
    };
    let syn::Expr::Reference(reference) = argument else {
        return false;
    };
    matches!(
        reference.expr.as_ref(),
        syn::Expr::Field(field)
            if matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("self"))
                && matches!(&field.member, syn::Member::Named(name) if name == "source")
    )
}
