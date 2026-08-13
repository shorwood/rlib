extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::ImplItem;
use rustc_lint::{LateContext, LateLintPass};

use super::utils::enumeration::CountCandidate;

/// Carries the `StrumManualEnumCounts` state used by this analysis.
struct StrumManualEnumCounts;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_COUNTS,
    Warn,
    "finds manually maintained enum variant totals reproducible by Strum",
    StrumManualEnumCounts
}

impl LateLintPass<'_> for StrumManualEnumCounts {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Prepare the values used by this stage.
        let Some(analyze_candidate) = CountCandidate::from_impl_item(cx, item) else {
            return;
        };
        let name = analyze_candidate.enum_name(cx);

        // Perform the next step of the analysis.
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_COUNTS,
            analyze_candidate.owner,
            analyze_candidate.span,
            DiagDecorator(|diag| {
                diag.primary_message(format!("`{name}` variant count is maintained manually"));
                diag.note("the literal total duplicates the enum definition and can become stale");
                if analyze_candidate.is_public_api() {
                    diag.note("this API is public, so replacing it with the `EnumCount` trait requires a compatibility review");
                }
                diag.help("derive `strum::EnumCount`, import its trait, and use `Type::COUNT`");
            }),
        );
    }
}
