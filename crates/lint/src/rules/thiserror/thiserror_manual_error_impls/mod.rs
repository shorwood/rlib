extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::utils::error_implementations::ManualErrorCatalog;
use crate::rules::framework::utils::config::{DeriveResolutionConfig, ErrorImplementationProvider};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Manual complete error contract
// -----------------------------------------------------------------------------

/// Static presentation and conventional source behavior derivable by thiserror.
struct Violation {
    /// Error type receiving the diagnostic.
    span: Span,
    /// Authored error type name.
    name: String,
    /// Static display message.
    message: String,
    /// Conventionally forwarded source field, when present.
    source_field: Option<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual `Display` and `Error` implementations for `{}` are derivable with thiserror",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the authored implementations only provide a static message and conventional source forwarding",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        let source = self.source_field.as_deref().map_or(String::new(), |field| {
            format!(" and mark `{field}` with `#[source]`")
        });
        Cow::Owned(format!(
            "derive `thiserror::Error`, add `#[error({:?})]`{source}, and remove both implementations",
            self.message
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_MANUAL_ERROR_IMPLS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this complete error contract is derivable");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// ThiserrorManualErrorImpls: Configured complete-error derive policy
// -----------------------------------------------------------------------------

/// Reports complete manual contracts when thiserror is the selected provider.
struct ThiserrorManualErrorImpls {
    /// Manual display and error implementations correlated by target type.
    catalog: ManualErrorCatalog,
    /// Explicit provider selection shared with overlapping derive frameworks.
    config: DeriveResolutionConfig,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_MANUAL_ERROR_IMPLS,
    Warn,
    "finds complete error contracts reproducible by thiserror",
    ThiserrorManualErrorImpls::new()
}

impl ThiserrorManualErrorImpls {
    /// Loads explicit derive-provider resolution.
    fn new() -> Self {
        Self {
            catalog: ManualErrorCatalog::default(),
            config: LibraryConfig::load().derive_resolution,
        }
    }

    /// Returns whether thiserror owns this complete error contract.
    fn uses_thiserror_provider(&self, source_field: Option<&str>) -> bool {
        if cfg!(feature = "derive_more") && source_field.is_none_or(|field| field == "source") {
            self.config.error_implementation() == Some(ErrorImplementationProvider::ThiserrorError)
        } else {
            true
        }
    }
}
impl LateLintPass<'_> for ThiserrorManualErrorImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.catalog.candidates() {
            if !self.uses_thiserror_provider(candidate.source_field.as_deref()) {
                continue;
            }

            Violation {
                span: candidate.span,
                name: candidate.name,
                message: candidate.message,
                source_field: candidate.source_field,
            }
            .emit(cx);
        }
    }
}
