extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};

use super::utils::enumeration::{CollectionCandidate, CollectionProvider};
use crate::utils::config::LibraryConfig;

/// Finds authored variant arrays that `VariantArray` can generate without changing order.
struct StrumManualVariantArrays {
    /// Explicitly resolved framework provider, when one is available.
    provider: Option<CollectionProvider>,
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
        let name = candidate.enum_name(cx);

        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_VARIANT_ARRAYS,
            candidate.owner,
            candidate.span,
            DiagDecorator(|diag| {
                diag.primary_message(format!("`{name}` variants are repeated in a manual array"));
                diag.note("the exhaustive declaration-order array duplicates the enum definition and can become stale");
                if candidate.is_public_api() {
                    diag.note("this API is public; `VariantArray::VARIANTS` is a shared slice, so migration requires a compatibility review");
                }
                diag.help("derive `strum::VariantArray`, import its trait, and migrate callers to `Type::VARIANTS`");
            }),
        );
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_VARIANT_ARRAYS,
    Warn,
    "finds manual exhaustive variant arrays reproducible by Strum",
    StrumManualVariantArrays::new()
}

impl LateLintPass<'_> for StrumManualVariantArrays {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.check(cx, CollectionCandidate::from_item(cx, item));
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        self.check(cx, CollectionCandidate::from_impl_item(cx, item));
    }
}
