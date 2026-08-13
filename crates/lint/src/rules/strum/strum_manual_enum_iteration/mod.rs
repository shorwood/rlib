extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::enumeration::{CollectionCandidate, CollectionProvider};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `enum_name` value used by this analysis.
    enum_name: Symbol,
    /// Stores the `is_public_api` value used by this analysis.
    is_public_api: bool,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` variants are manually exposed as an iterator",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the exhaustive declaration-order sequence duplicates the enum definition and can become stale",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive `strum::EnumIter`, import `strum::IntoEnumIterator`, and migrate callers to `Type::iter()`",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Prepare the values used by this stage.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Perform the next step of the analysis.
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_ITERATION,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.note(rationale);
                if self.is_public_api {
                    diag.note("this API is public, so changing its return type or generated iterator name requires a compatibility review");
                }
                diag.help(remediation);
            }),
        );
    }
}

/// Carries the `StrumManualEnumIteration` state used by this analysis.
struct StrumManualEnumIteration {
    /// Stores the `provider` value used by this analysis.
    provider: Option<CollectionProvider>,
}

impl StrumManualEnumIteration {
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
        if !(analyze_candidate.selected(self.provider) == Some(CollectionProvider::StrumEnumIter)) {
            return;
        }

        // Perform the next step of the analysis.
        Violation {
            span: analyze_candidate.span,
            owner: analyze_candidate.owner,
            enum_name: analyze_candidate.enum_name(cx),
            is_public_api: analyze_candidate.is_public_api(),
        }
        .emit(cx);
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_ITERATION,
    Warn,
    "finds manual exhaustive enum iteration reproducible by Strum",
    StrumManualEnumIteration::new()
}

impl LateLintPass<'_> for StrumManualEnumIteration {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.check(cx, CollectionCandidate::from_item(cx, item));
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        self.check(cx, CollectionCandidate::from_impl_item(cx, item));
    }
}
