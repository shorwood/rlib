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
use crate::rules::thiserror::manual_error::Catalog as ManualErrorCatalog;
#[cfg(feature = "thiserror")]
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `owner` value used by this analysis.
    owner: HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
}

impl Candidate {
    /// Performs the `analyze_candidate` step of the lint analysis.
    fn analyze_candidate(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };
        if item.span.from_expansion() || !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return None;
        }

        // Prepare the values used by this stage.
        let trait_id = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;
        if cx.tcx.item_name(trait_id).as_str() != "Error"
            || !matches!(cx.tcx.crate_name(trait_id.krate).as_str(), "core" | "std")
        // Perform the next step of the analysis.
        {
            return None;
        }
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();

        // Prepare the values used by this stage.
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

        // Return the completed analysis result.
        Some(Self {
            definition: definition.did().as_local()?,
            owner: item.hir_id(),
            span: item.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
        })
    }
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
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

/// Performs the `conventional_source` step of the lint analysis.
fn conventional_source(method: &syn::ImplItemFn) -> bool {
    // Reject inputs that do not satisfy this stage.
    if method.sig.ident != "source" {
        return false;
    }
    let [syn::Stmt::Expr(syn::Expr::Call(call), _)] = method.block.stmts.as_slice() else {
        return false;
    };

    // Reject inputs that do not satisfy this stage.
    if !matches!(call.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Some")) {
        return false;
    }
    if call.args.len() != 1 {
        return false;
    }

    // Prepare the values used by this stage.
    let Some(argument) = call.args.first() else {
        return false;
    };

    // Prepare the values used by this stage.
    let syn::Expr::Reference(reference) = argument else {
        return false;
    };

    // Perform the next step of the analysis.

    // Perform the next step of the analysis.
    matches!(
        reference.expr.as_ref(),
        syn::Expr::Field(field)
            if matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("self"))
                && matches!(&field.member, syn::Member::Named(name) if name == "source")
    )
}

/// Performs the `is_derivable_error_impl` step of the lint analysis.
fn is_derivable_error_impl(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
    // Prepare the values used by this stage.
    let Some(source) = AuthoredItemSource::for_item(cx, item) else {
        return false;
    };
    let Ok(implementation) = syn::parse_str::<syn::ItemImpl>(&source) else {
        return false;
    };

    // Classify the current analyze_candidate.
    match implementation.items.as_slice() {
        [] => true,
        [syn::ImplItem::Fn(method)] => conventional_source(method),
        _ => false,
    }
}

/// Carries the `DeriveMoreManualErrorImpls` state used by this analysis.
struct DeriveMoreManualErrorImpls {
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
    #[cfg(feature = "thiserror")]
    /// Stores the `config` value used by this analysis.
    config: DeriveResolutionConfig,
    #[cfg(feature = "thiserror")]
    /// Stores the `overlaps` value used by this analysis.
    overlaps: ManualErrorCatalog,
}

impl DeriveMoreManualErrorImpls {
    /// Performs the `new` operation for this value.
    fn new() -> Self {
        Self {
            candidates: Vec::new(),
            #[cfg(feature = "thiserror")]
            config: LibraryConfig::load().derive_resolution,
            #[cfg(feature = "thiserror")]
            overlaps: ManualErrorCatalog::default(),
        }
    }

    /// Performs the `selected` operation for this value.
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
        let Some(analyze_candidate) = Candidate::analyze_candidate(cx, item) else {
            return;
        };
        self.candidates.push(analyze_candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in mem::take(&mut self.candidates) {
            if !self.selected(analyze_candidate.definition) {
                continue;
            }
            Violation {
                owner: analyze_candidate.owner,
                span: analyze_candidate.span,
                name: analyze_candidate.name,
            }
            .emit(cx);
        }
    }
}
