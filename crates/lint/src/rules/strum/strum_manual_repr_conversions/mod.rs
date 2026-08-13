extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::ImplItem;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::variant_methods::ReprConversionCandidate;

// -----------------------------------------------------------------------------
// Violation: Authored representation conversion
// -----------------------------------------------------------------------------

/// Exact integer-to-variant conversion reproducible by `FromRepr`.
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
            "`{}::from_repr` repeats the enum discriminant mapping",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the exhaustive integer-to-unit-variant match duplicates resolved discriminants and `Option` failure behavior",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("derive `strum::FromRepr` and remove the equivalent inherent conversion")
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_REPR_CONVERSIONS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.note(rationale);
                if self.is_public_api {
                    diag.note("this conversion is public; generated visibility, constness, and signature require a compatibility review");
                }
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// StrumManualReprConversions: Discriminant match analysis
// -----------------------------------------------------------------------------

/// Finds manual integer discriminant conversions reproducible by `FromRepr`.
struct StrumManualReprConversions;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_REPR_CONVERSIONS,
    Warn,
    "finds manual enum representation conversions reproducible by Strum",
    StrumManualReprConversions
}

impl LateLintPass<'_> for StrumManualReprConversions {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let Some(candidate) = ReprConversionCandidate::from_impl_item(cx, item) else {
            return;
        };

        Violation {
            span: candidate.span,
            owner: candidate.owner,
            enum_name: candidate.enum_name(cx),
            is_public_api: candidate.is_public_api(),
        }
        .emit(cx);
    }
}
