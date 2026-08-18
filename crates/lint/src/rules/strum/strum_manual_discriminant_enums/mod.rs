extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{ImplItem, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::DiscriminantMirrorCandidate;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Hand-maintained discriminant mirror
// -----------------------------------------------------------------------------

/// Secondary enum that manually mirrors a source enum's variants.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Authored source used to recover framework metadata.
    source: Symbol,
    /// Name of the companion enum duplicating the source enum's discriminants.
    mirror: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` manually mirrors `{}`",
            self.mirror, self.source
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the private unit enum and conversion duplicate every source variant one-for-one",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derive `strum::EnumDiscriminants` and configure its generated name as `{}`",
            self.mirror
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_DISCRIMINANT_ENUMS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// StrumManualDiscriminantEnums: Generated discriminant mirror policy
// -----------------------------------------------------------------------------

/// Detects discriminant mirrors reproducible by Strum.
#[derive(Default)]
struct StrumManualDiscriminantEnums {
    /// Exact manual mirrors awaiting crate-wide schema evidence.
    candidates: Vec<DiscriminantMirrorCandidate>,
    /// Mirror enums with authored or generated Serde contracts.
    external_schemas: HashSet<LocalDefId>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_DISCRIMINANT_ENUMS,
    Warn,
    "finds manually mirrored enum discriminants reproducible by Strum",
    StrumManualDiscriminantEnums::default()
}

impl LateLintPass<'_> for StrumManualDiscriminantEnums {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Only implementation blocks can establish an external schema for a mirror enum.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };

        // Inherent implementations do not bind the enum to an external serialization schema.
        let Some(trait_def) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };

        // Traits outside Serde's serialization contract do not constrain discriminant shape.
        if !matches!(
            cx.tcx.item_name(trait_def).as_str(),
            "Serialize" | "Deserialize"
        ) || !matches!(
            cx.tcx.crate_name(trait_def.krate).as_str(),
            "serde" | "serde_core"
        ) {
            return;
        }

        // Implementations without a local aggregate target cannot mark a local mirror enum.
        let Some(definition) = cx
            .tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        else {
            return;
        };
        self.external_schemas.insert(definition);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Methods that do not mirror enum discriminants are outside this lint's candidate model.
        let Some(candidate) = DiscriminantMirrorCandidate::from_impl_item(cx, item) else {
            return;
        };

        self.candidates.push(candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self.external_schemas.contains(&candidate.mirror_enum) {
                continue;
            }
            Violation {
                owner: candidate.owner,
                span: candidate.span,
                source: cx.tcx.item_name(candidate.source_enum.to_def_id()),
                mirror: cx.tcx.item_name(candidate.mirror_enum.to_def_id()),
            }
            .emit(cx);
        }
    }
}
