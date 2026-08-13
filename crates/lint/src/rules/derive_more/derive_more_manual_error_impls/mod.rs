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
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Candidate: Derivable manual error implementation evidence
// -----------------------------------------------------------------------------

/// Manual error implementation awaiting source-field and framework ownership checks.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Declaration whose lint level governs this finding.
    owner: HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
}

impl Candidate {
    /// Recovers the authored error implementation and its target type.
    fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
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

        Some(Self {
            definition: definition.did().as_local()?,
            owner: item.hir_id(),
            span: item.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
        })
    }
}

// -----------------------------------------------------------------------------
// Violation: Derivable manual error implementation
// -----------------------------------------------------------------------------

/// Error implementation proven equivalent to `derive_more`'s generated behavior.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
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

/// Finds the unique field conventionally acting as an error source.
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

/// Proves that `source` only returns the conventional field as a trait object.
fn is_derivable_error_impl(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
    let Some(source) = AuthoredItemSource::for_item(cx, item) else {
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

// -----------------------------------------------------------------------------
// DeriveMoreManualErrorImpls: Declarative error policy
// -----------------------------------------------------------------------------

/// Correlates derivable `Error` implementations with overlapping thiserror policy.
struct DeriveMoreManualErrorImpls {
    /// Authored declarations awaiting association with `derive_more` expansions.
    candidates: Vec<Candidate>,
    #[cfg(feature = "thiserror")]
    /// Validated project policy applied by this lint pass.
    config: DeriveResolutionConfig,
    #[cfg(feature = "thiserror")]
    /// Types that thiserror already owns under the configured framework resolution.
    overlaps: ManualErrorCatalog,
}

impl DeriveMoreManualErrorImpls {
    /// Starts error analysis with no manual implementations or framework overlaps.
    fn new() -> Self {
        Self {
            candidates: Vec::new(),
            #[cfg(feature = "thiserror")]
            config: LibraryConfig::load().derive_resolution,
            #[cfg(feature = "thiserror")]
            overlaps: ManualErrorCatalog::default(),
        }
    }

    /// Resolves the configured provider when several implementations are available.
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
        let Some(candidate) = Candidate::from_item(cx, item) else {
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
