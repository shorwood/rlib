extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::ImplItem;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Symbol;

use super::utils::enumeration::CountCandidate;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Manually maintained enum cardinality diagnostic
// -----------------------------------------------------------------------------

/// Exact enum count that Strum can derive from the enum declaration.
struct Violation {
    /// Authored count declaration and compatibility evidence.
    candidate: CountCandidate,
    /// Owning enum name resolved while compiler context is available.
    enum_name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` variant count is maintained manually",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("the literal total duplicates the enum definition and can become stale")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("derive `strum::EnumCount`, import its trait, and use `Type::COUNT`")
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();

        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_COUNTS,
            self.candidate.owner,
            self.candidate.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                diag.note(rationale_message);
                if self.candidate.is_public_api() {
                    diag.note("this API is public, so replacing it with the `EnumCount` trait requires a compatibility review");
                }
                diag.help(remediation_message);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// StrumManualEnumCounts: EnumCount adoption policy
// -----------------------------------------------------------------------------

/// Finds fixed enum counts that `EnumCount` can derive from the declaration.
struct StrumManualEnumCounts;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_COUNTS,
    Warn,
    "finds manually maintained enum variant totals reproducible by Strum",
    StrumManualEnumCounts
}

impl LateLintPass<'_> for StrumManualEnumCounts {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Implementation items without a complete manual count are unrelated.
        let Some(candidate) = CountCandidate::from_impl_item(cx, item) else {
            return;
        };
        let enum_name = candidate.enum_name(cx);
        Violation {
            candidate,
            enum_name,
        }
        .emit(cx);
    }
}
