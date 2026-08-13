extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Symbol;

use super::utils::enumeration::{CollectionCandidate, CollectionProvider};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Manual exhaustive variant collection diagnostic
// -----------------------------------------------------------------------------

/// Exhaustive enum collection that `VariantArray` can derive without changing order.
struct Violation {
    /// Authored collection contract and compatibility evidence.
    candidate: CollectionCandidate,
    /// Owning enum name resolved while compiler context is available.
    enum_name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` variants are repeated in a manual array",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the exhaustive declaration-order array duplicates the enum definition and can become stale",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive `strum::VariantArray`, import its trait, and migrate callers to `Type::VARIANTS`",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();

        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_VARIANT_ARRAYS,
            self.candidate.owner,
            self.candidate.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                diag.note(rationale_message);
                if self.candidate.is_public_api() {
                    diag.note("this API is public; `VariantArray::VARIANTS` is a shared slice, so migration requires a compatibility review");
                }
                diag.help(remediation_message);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// StrumManualVariantArrays: Selected VariantArray adoption policy
// -----------------------------------------------------------------------------

/// Finds authored variant arrays that `VariantArray` can generate without changing order.
struct StrumManualVariantArrays {
    /// Explicitly resolved framework provider, when one is available.
    provider: Option<CollectionProvider>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_VARIANT_ARRAYS,
    Warn,
    "finds manual exhaustive variant arrays reproducible by Strum",
    StrumManualVariantArrays::new()
}

impl StrumManualVariantArrays {
    /// Starts variant-array analysis with the configured framework owner.
    fn new() -> Self {
        Self {
            provider: LibraryConfig::load()
                .derive_resolution
                .enum_variant_collection(),
        }
    }

    /// Reports a complete authored variant array when Strum owns its replacement.
    fn check(&self, cx: &LateContext<'_>, candidate: Option<CollectionCandidate>) {
        let Some(candidate) = candidate else {
            return;
        };
        if candidate.selected(self.provider) != Some(CollectionProvider::StrumVariantArray) {
            return;
        }
        let enum_name = candidate.enum_name(cx);
        Violation {
            candidate,
            enum_name,
        }
        .emit(cx);
    }
}
impl LateLintPass<'_> for StrumManualVariantArrays {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.check(cx, CollectionCandidate::from_item(cx, item));
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        self.check(cx, CollectionCandidate::from_impl_item(cx, item));
    }
}
