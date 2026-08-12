extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::ImplItem;
use rustc_lint::{LateContext, LateLintPass};

use super::utils::enumeration::CountCandidate;

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
        let Some(candidate) = CountCandidate::from_impl_item(cx, item) else {
            return;
        };
        let name = candidate.enum_name(cx);
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_COUNTS,
            candidate.owner,
            candidate.span,
            DiagDecorator(|diag| {
                diag.primary_message(format!("`{name}` variant count is maintained manually"));
                diag.note("the literal total duplicates the enum definition and can become stale");
                if candidate.is_public_api() {
                    diag.note("this API is public, so replacing it with the `EnumCount` trait requires a compatibility review");
                }
                diag.help("derive `strum::EnumCount`, import its trait, and use `Type::COUNT`");
            }),
        );
    }
}
