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

// -----------------------------------------------------------------------------
// Violation: Hand-maintained complete enum collection
// -----------------------------------------------------------------------------

/// Complete variant collection reproducible by the configured Strum provider.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Enum name quoted in the diagnostic.
    enum_name: Symbol,
    /// Whether replacement would change a public API.
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
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

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

// -----------------------------------------------------------------------------
// StrumManualEnumIteration: Generated complete-iteration policy
// -----------------------------------------------------------------------------

/// Finds complete manual variant collections after provider resolution.
struct StrumManualEnumIteration {
    /// Explicitly resolved framework provider, when one is available.
    provider: Option<CollectionProvider>,
}

impl StrumManualEnumIteration {
    /// Starts enum-iteration analysis with the configured framework owner.
    fn new() -> Self {
        Self {
            provider: LibraryConfig::load()
                .derive_resolution
                .enum_variant_collection(),
        }
    }

    /// Reports complete authored iteration when Strum owns its replacement.
    fn check(&self, cx: &LateContext<'_>, candidate: Option<CollectionCandidate>) {
        let Some(candidate) = candidate else {
            return;
        };
        if !(candidate.selected(self.provider) == Some(CollectionProvider::StrumEnumIter)) {
            return;
        }

        Violation {
            span: candidate.span,
            owner: candidate.owner,
            enum_name: candidate.enum_name(cx),
            is_public_api: candidate.is_public_api(),
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
