extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::BTreeSet;

use malva::{Syntax, format_text};
use rustc_ast::ast::{Crate, Item};
use rustc_errors::{Applicability, DiagDecorator};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::rules::leptos::utils::authored_files::{AuthoredFiles, SourceDocument};
use crate::rules::leptos_styling::utils::config::LeptosStylingCssFormattingConfig;
use crate::rules::leptos_styling::utils::source::ComponentStyleAnalysis;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Noncanonical or malformed external CSS
// -----------------------------------------------------------------------------

/// One registered stylesheet that Malva rejects or would rewrite.
struct Violation {
    /// Complete external stylesheet span receiving the diagnostic and optional replacement.
    span: Span,
    /// Parser or formatter failure explaining why no canonical replacement exists.
    error: Option<String>,
    /// Complete canonically formatted stylesheet source when formatting succeeds.
    replacement: Option<String>,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        self.error.as_ref().map_or_else(
            || Cow::Borrowed("component CSS is not canonically formatted"),
            |error| Cow::Owned(format!("component CSS is malformed: {error}")),
        )
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "strict parsing and deterministic formatting keep external component styles reviewable",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        if self.replacement.is_some() {
            Cow::Borrowed("format this stylesheet with the configured Malva policy")
        } else {
            Cow::Borrowed("repair the CSS syntax before formatting it")
        }
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_STYLING_NONCANONICAL_CSS,
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
// LeptosStylingNoncanonicalCss: Malva-backed CSS policy
// -----------------------------------------------------------------------------

/// Parses and formats every stylesheet registered through `style_sheet!`.
struct LeptosStylingNoncanonicalCss {
    /// Project CSS formatting policy passed to Malva.
    config: LeptosStylingCssFormattingConfig,
    /// Authored source files registering stylesheets, deduplicated across callbacks.
    files: AuthoredFiles,
}

impl LeptosStylingNoncanonicalCss {
    /// Builds a formatter pass from the project configuration.
    fn new() -> Self {
        Self {
            config: LeptosStylingCssFormattingConfig::from_config(),
            files: AuthoredFiles::default(),
        }
    }
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_STYLING_NONCANONICAL_CSS,
    Warn,
    "requires strictly parsed and canonically formatted component CSS",
    LeptosStylingNoncanonicalCss::new()
}

impl EarlyLintPass for LeptosStylingNoncanonicalCss {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _krate: &Crate) {
        let options = self.config.options();
        let mut seen = BTreeSet::new();
        for document in self.files.documents(cx) {
            let Some(analysis) = ComponentStyleAnalysis::analyze(document) else {
                continue;
            };
            for style_sheet in analysis.style_sheets {
                let Some(path) = style_sheet.declared else {
                    continue;
                };
                if !seen.insert(path.clone()) {
                    continue;
                }
                let Some(css) = SourceDocument::load(cx, &path) else {
                    continue;
                };
                let span = css.complete_span();
                match format_text(&css.source, Syntax::Css, &options) {
                    Ok(formatted) if formatted != css.source => Violation {
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
}
