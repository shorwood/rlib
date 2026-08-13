extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::contracts::ThiserrorContractCatalog;
use super::manual_from::Candidate as ManualFromCandidate;
use crate::rules::framework::config::{DeriveResolutionConfig, ErrorVariantConversionProvider};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `error` value used by this analysis.
    error: String,
    /// Stores the `variant` value used by this analysis.
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

/// Carries the `ThiserrorManualFromErrorVariants` state used by this analysis.
struct ThiserrorManualFromErrorVariants {
    /// Stores the `catalog` value used by this analysis.
    catalog: ThiserrorContractCatalog,
    /// Stores the `config` value used by this analysis.
    config: DeriveResolutionConfig,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<ManualFromCandidate>,
}

impl ThiserrorManualFromErrorVariants {
    /// Performs the `new` operation for this value.
    fn new() -> Self {
        Self {
            catalog: ThiserrorContractCatalog::default(),
            config: LibraryConfig::load().derive_resolution,
            candidates: Vec::new(),
        }
    }

    /// Performs the `selected` operation for this value.
    fn selected(&self) -> bool {
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

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_MANUAL_FROM_ERROR_VARIANTS,
    Warn,
    "finds error-variant conversions reproducible by thiserror",
    ThiserrorManualFromErrorVariants::new()
}

impl LateLintPass<'_> for ThiserrorManualFromErrorVariants {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let Some(analyze_candidate) = ManualFromCandidate::from_impl_item(cx, item) else {
            return;
        };
        self.candidates.push(analyze_candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        if !self.selected() {
            return;
        }
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition)
                .is_none()
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                error: analyze_candidate.error,
                variant: analyze_candidate.variant,
            }
            .emit(cx);
        }
    }
}
