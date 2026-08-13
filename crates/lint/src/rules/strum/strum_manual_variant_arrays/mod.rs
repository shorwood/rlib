extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};

use super::utils::enumeration::{CollectionCandidate, CollectionProvider};
use crate::utils::config::LibraryConfig;

/// Carries the `StrumManualVariantArrays` state used by this analysis.
struct StrumManualVariantArrays {
    /// Stores the `provider` value used by this analysis.
    provider: Option<CollectionProvider>,
}

impl StrumManualVariantArrays {
    /// Performs the `new` operation for this value.
    fn new() -> Self {
        Self {
            provider: LibraryConfig::load()
                .derive_resolution
                .enum_variant_collection(),
        }
    }

    /// Performs the `check` operation for this value.
    fn check(&self, cx: &LateContext<'_>, analyze_candidate: Option<CollectionCandidate>) {
        // Prepare the values used by this stage.
        let Some(analyze_candidate) = analyze_candidate else {
            return;
        };
        if analyze_candidate.selected(self.provider) != Some(CollectionProvider::StrumVariantArray)
        {
            return;
        }
        let name = analyze_candidate.enum_name(cx);

        // Perform the next step of the analysis.
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_VARIANT_ARRAYS,
            analyze_candidate.owner,
            analyze_candidate.span,
            DiagDecorator(|diag| {
                diag.primary_message(format!("`{name}` variants are repeated in a manual array"));
                diag.note("the exhaustive declaration-order array duplicates the enum definition and can become stale");
                if analyze_candidate.is_public_api() {
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
