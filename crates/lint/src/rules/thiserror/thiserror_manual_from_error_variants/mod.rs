extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::utils::contracts::ThiserrorContractCatalog;
use super::utils::error_conversions::Candidate as ManualFromCandidate;
use crate::rules::framework::utils::config::{
    DeriveResolutionConfig, ErrorVariantConversionProvider,
};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Manual conversion into a thiserror variant
// -----------------------------------------------------------------------------

/// Exact source wrapper that thiserror can derive with `#[from]`.
struct Violation {
    /// Manual implementation receiving the diagnostic.
    span: Span,
    /// Target error enum.
    error: String,
    /// Variant that wraps the source value.
    variant: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual conversion into `{}::{}` is derivable with thiserror",
            self.error, self.variant
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the implementation only wraps the source in the variant, duplicating thiserror's source and conversion policy",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("remove the implementation and mark the variant field `#[from]`")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_MANUAL_FROM_ERROR_VARIANTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this conversion is exact error-variant wrapping");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// ThiserrorManualFromErrorVariants: Configured conversion provider policy
// -----------------------------------------------------------------------------

/// Reports derivable conversions when thiserror owns variant conversion policy.
struct ThiserrorManualFromErrorVariants {
    /// Local derived error contracts.
    catalog: ThiserrorContractCatalog,
    /// Explicit provider selection shared with overlapping derive frameworks.
    config: DeriveResolutionConfig,
    /// Exact manual wrappers awaiting derive confirmation.
    candidates: Vec<ManualFromCandidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_MANUAL_FROM_ERROR_VARIANTS,
    Warn,
    "finds error-variant conversions reproducible by thiserror",
    ThiserrorManualFromErrorVariants::new()
}

impl ThiserrorManualFromErrorVariants {
    /// Loads explicit derive-provider resolution.
    fn new() -> Self {
        Self {
            catalog: ThiserrorContractCatalog::default(),
            config: LibraryConfig::load().derive_resolution,
            candidates: Vec::new(),
        }
    }

    /// Returns whether configuration assigns this conversion to thiserror.
    fn uses_thiserror_provider(&self) -> bool {
        if cfg!(feature = "derive_more") {
            self.config.error_variant_conversion()
                == Some(ErrorVariantConversionProvider::ThiserrorFrom)
        } else {
            self.config
                .error_variant_conversion()
                .is_none_or(|provider| provider == ErrorVariantConversionProvider::ThiserrorFrom)
        }
    }
}
impl LateLintPass<'_> for ThiserrorManualFromErrorVariants {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Implementation items without a complete error conversion are unrelated.
        let Some(candidate) = ManualFromCandidate::from_impl_item(cx, item) else {
            return;
        };
        self.candidates.push(candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        // Manual conversions are replaceable only when Thiserror owns the error contract.
        if !self.uses_thiserror_provider() {
            return;
        }
        for candidate in self.candidates.drain(..) {
            if self.catalog.derived_type(candidate.definition).is_none() {
                continue;
            }

            Violation {
                span: candidate.span,
                error: candidate.error,
                variant: candidate.variant,
            }
            .emit(cx);
        }
    }
}
