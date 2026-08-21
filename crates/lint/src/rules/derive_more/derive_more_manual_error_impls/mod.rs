extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::mem;

use rustc_errors::DiagDecorator;
use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{ExprKind, HirId, ImplItemKind, Item, ItemKind, Mutability, PatKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::symbol::sym;

#[cfg(feature = "thiserror")]
use crate::config::{
    framework::DeriveResolutionConfig, providers::ErrorImplementationProvider, store::ConfigStore,
};
#[cfg(feature = "thiserror")]
use crate::rules::thiserror::utils::error_implementations::ManualErrorCatalog;
use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

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
        // Only implementation items can provide an Error trait contract.
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };

        // Generated or attributed implementations may carry behavior a derive cannot preserve.
        if item.span.from_expansion() || !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return None;
        }

        let trait_id = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;

        // Only the standard error trait has the derive_more replacement contract.
        if cx.tcx.item_name(trait_id).as_str() != "Error"
            || !matches!(cx.tcx.crate_name(trait_id.krate).as_str(), "core" | "std")
        {
            return None;
        }
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();

        // Only nominal self types can receive an Error derive.
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return None;
        };

        // Only concrete local structs whose implementation adds no behavior are derivable.
        if !definition.is_struct()
            || definition.did().as_local().is_none()
            || !cx.tcx.generics_of(definition.did()).own_params.is_empty()
            || !DeriveMoreManualErrorImpls::is_derivable_error_impl(cx, item, *definition)
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

// -----------------------------------------------------------------------------
// DeriveMoreManualErrorImpls: Declarative error policy
// -----------------------------------------------------------------------------

/// Correlates derivable `Error` implementations with overlapping thiserror policy.
struct DeriveMoreManualErrorImpls {
    /// Authored declarations awaiting association with `derive_more` expansions.
    candidates: Vec<Candidate>,
    /// Validated project policy applied by this lint pass.
    #[cfg(feature = "thiserror")]
    config: DeriveResolutionConfig,
    /// Types that thiserror already owns under the configured framework resolution.
    #[cfg(feature = "thiserror")]
    overlaps: ManualErrorCatalog,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_ERROR_IMPLS,
    Warn,
    "finds Error implementations reproducible by derive_more",
    DeriveMoreManualErrorImpls::new()
}

impl DeriveMoreManualErrorImpls {
    /// Starts error analysis with no manual implementations or framework overlaps.
    fn new() -> Self {
        Self {
            candidates: Vec::new(),
            #[cfg(feature = "thiserror")]
            config: ConfigStore::get().derive_resolution.clone(),
            #[cfg(feature = "thiserror")]
            overlaps: ManualErrorCatalog::default(),
        }
    }

    /// Returns whether `derive_more` would add source or backtrace behavior to an empty impl.
    fn is_derive_adding_behavior(cx: &LateContext<'_>, definition: ty::AdtDef<'_>) -> bool {
        let fields = &definition.non_enum_variant().fields;
        let is_tuple = fields
            .iter()
            .all(|field| field.name.as_str().parse::<usize>().is_ok());
        fields.iter().any(|field| {
            let name = field.name.as_str();
            let is_backtrace = name == "backtrace"
                || field
                    .ty(cx.tcx, ty::GenericArgs::empty())
                    .ty_adt_def()
                    .is_some_and(|adt| cx.tcx.item_name(adt.did()).as_str() == "Backtrace");
            name == "source" || is_backtrace || (is_tuple && fields.len() == 1)
        })
    }

    /// Finds the unique field conventionally acting as an error source.
    fn has_conventional_source(cx: &LateContext<'_>, method: &rustc_hir::ImplItem<'_>) -> bool {
        // Only an unattributed source method can match derive_more's generated behavior.
        if method.ident.name.as_str() != "source" || !cx.tcx.hir_attrs(method.hir_id()).is_empty() {
            return false;
        }

        // Non-function associated items cannot implement source forwarding.
        let ImplItemKind::Fn(_, body_id) = method.kind else {
            return false;
        };
        let body = cx.tcx.hir_body(body_id);

        // Source methods with other arities cannot forward only their receiver.
        let [parameter] = body.params else {
            return false;
        };

        // Only a simple receiver binding can be tracked to the returned field.
        let PatKind::Binding(_, receiver, _, None) = parameter.pat.kind else {
            return false;
        };

        // Multi-expression bodies contain behavior beyond direct source forwarding.
        let Some(expression) = DirectForwarding::single_body_expression(body.value) else {
            return false;
        };

        // The generated shape is exactly a one-argument Option constructor call.
        let ExprKind::Call(callee, [argument]) = expression.kind else {
            return false;
        };

        // Indirect callees cannot prove construction of the standard Some variant.
        let ExprKind::Path(path) = callee.kind else {
            return false;
        };

        // Only a resolved enum-variant constructor can be the Some wrapper.
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            cx.qpath_res(&path, callee.hir_id)
        else {
            return false;
        };
        let variant = cx.tcx.parent(constructor);

        // Only the standard Option::Some constructor matches generated source behavior.
        if cx.tcx.item_name(variant).as_str() != "Some"
            || !cx
                .tcx
                .is_diagnostic_item(sym::Option, cx.tcx.parent(variant))
        {
            return false;
        }

        // The source must be returned through an immutable reference.
        let ExprKind::AddrOf(_, Mutability::Not, field) = argument.kind else {
            return false;
        };

        // Only direct field access can prove conventional source forwarding.
        let ExprKind::Field(base, name) = field.kind else {
            return false;
        };
        name.name.as_str() == "source" && DirectForwarding::is_binding(cx, base, receiver)
    }

    /// Proves that `source` only returns the conventional field as a trait object.
    fn is_derivable_error_impl(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        definition: ty::AdtDef<'_>,
    ) -> bool {
        // Only implementation items can be checked for a derivable Error body.
        let ItemKind::Impl(implementation) = item.kind else {
            return false;
        };
        match implementation.items {
            [] => !Self::is_derive_adding_behavior(cx, definition),
            [id] => Self::has_conventional_source(cx, cx.tcx.hir_impl_item(*id)),
            _ => false,
        }
    }

    /// Resolves the configured provider when several implementations are available.
    fn is_selected(&self, definition: LocalDefId) -> bool {
        #[cfg(feature = "thiserror")]
        // Overlapping providers report only when configuration selects derive_more.
        if self.overlaps.contains(definition) {
            return self.config.error_implementation()
                == Some(ErrorImplementationProvider::DeriveMoreError);
        }
        let _ = definition;
        true
    }
}

impl LateLintPass<'_> for DeriveMoreManualErrorImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        #[cfg(feature = "thiserror")]
        self.overlaps.check_item(cx, item);

        // Items without a fully derivable Error implementation are not candidates.
        let Some(candidate) = Candidate::from_item(cx, item) else {
            return;
        };
        self.candidates.push(candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in mem::take(&mut self.candidates) {
            if !self.is_selected(candidate.definition) {
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
