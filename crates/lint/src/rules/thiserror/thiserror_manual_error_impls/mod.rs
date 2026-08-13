extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::manual_error::Catalog as ManualErrorCatalog;
use crate::rules::framework::config::{DeriveResolutionConfig, ErrorImplementationProvider};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `message` value used by this analysis.
    message: String,
    /// Stores the `source_field` value used by this analysis.
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

/// Carries the `ThiserrorManualErrorImpls` state used by this analysis.
struct ThiserrorManualErrorImpls {
    /// Stores the `catalog` value used by this analysis.
    catalog: ManualErrorCatalog,
    /// Stores the `config` value used by this analysis.
    config: DeriveResolutionConfig,
}

impl ThiserrorManualErrorImpls {
    /// Performs the `new` operation for this value.
    fn new() -> Self {
        Self {
            catalog: ManualErrorCatalog::default(),
            config: LibraryConfig::load().derive_resolution,
        }
    }

    /// Performs the `selected` operation for this value.
    fn selected(&self, source_field: Option<&str>) -> bool {
        if cfg!(feature = "derive_more") && source_field.is_none_or(|field| field == "source") {
            self.config.error_implementation() == Some(ErrorImplementationProvider::ThiserrorError)
        } else {
            true
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_MANUAL_ERROR_IMPLS,
    Warn,
    "finds complete error contracts reproducible by thiserror",
    ThiserrorManualErrorImpls::new()
}

impl LateLintPass<'_> for ThiserrorManualErrorImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.catalog.candidates() {
            // Reject inputs that do not satisfy this stage.
            if !self.selected(analyze_candidate.source_field.as_deref()) {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                name: analyze_candidate.name,
                message: analyze_candidate.message,
                source_field: analyze_candidate.source_field,
            }
            .emit(cx);
        }
    }
}
