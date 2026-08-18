extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use leptosfmt_formatter::format_file_source;
use rustc_ast::ast::{Crate, Item};
use rustc_errors::{Applicability, DiagDecorator};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};

use crate::config::leptos::LeptosViewFormattingConfig;
use crate::config::store::ConfigStore;
use crate::rules::leptos::utils::authored_files::AuthoredFiles;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Noncanonical or malformed authored view source
// -----------------------------------------------------------------------------

/// One Rust file whose Leptos markup cannot be accepted as canonical.
struct Violation {
    /// Authored file span receiving the formatting diagnostic and optional replacement.
    span: rustc_span::Span,
    /// Formatter failure explaining why no canonical replacement could be produced.
    error: Option<String>,
    /// Complete canonically formatted file source when formatting succeeds.
    replacement: Option<String>,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        self.error.as_ref().map_or_else(
            || Cow::Borrowed("Leptos view markup is not canonically formatted"),
            |error| {
                Cow::Owned(format!(
                    "Leptos view markup could not be formatted: {error}"
                ))
            },
        )
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("one deterministic view layout keeps component markup easy to scan")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        if self.replacement.is_some() {
            Cow::Borrowed("format every `view!` macro in this file")
        } else {
            Cow::Borrowed("repair the malformed `view!` syntax before formatting it")
        }
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_NONCANONICAL_VIEW_FORMATTING,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.note(rationale);
                if let Some(replacement) = self.replacement {
                    diag.span_suggestion(
                        self.span,
                        remediation,
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosNoncanonicalViewFormatting: Leptosfmt-backed source policy
// -----------------------------------------------------------------------------

/// Requires every authored `view!` macro in a file to match leptosfmt output.
struct LeptosNoncanonicalViewFormatting {
    /// Project formatting policy passed to the Leptos formatter.
    config: LeptosViewFormattingConfig,
    /// Authored source files containing view macros, deduplicated across callbacks.
    files: AuthoredFiles,
}

impl LeptosNoncanonicalViewFormatting {
    /// Builds a formatter pass from the project configuration.
    fn new() -> Self {
        Self {
            config: ConfigStore::get().leptos_view_formatting.clone(),
            files: AuthoredFiles::default(),
        }
    }
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_NONCANONICAL_VIEW_FORMATTING,
    Warn,
    "requires canonical formatting for Leptos view macros",
    LeptosNoncanonicalViewFormatting::new()
}

impl EarlyLintPass for LeptosNoncanonicalViewFormatting {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _krate: &Crate) {
        let settings = self.config.settings();
        for document in self.files.documents(cx) {
            let span = document.complete_span();
            match format_file_source(&document.source, &settings) {
                Ok(formatted) if formatted != document.source => Violation {
                    span,
                    error: None,
                    replacement: Some(formatted),
                }
                .emit(cx),
                Ok(_) => {}
                Err(error) => Violation {
                    span,
                    error: Some(error.to_string()),
                    replacement: None,
                }
                .emit(cx),
            }
        }
    }
}
