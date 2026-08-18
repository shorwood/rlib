extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Crate, Item};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::rules::leptos::utils::authored_files::AuthoredFiles;
use crate::rules::leptos_styling::utils::source::ComponentStyleAnalysis;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Direct inline presentation property
// -----------------------------------------------------------------------------

/// One authored style attribute that does more than bind a CSS custom property.
struct Violation {
    /// Authored attribute range receiving the diagnostic.
    span: Span,
    /// Policy-specific explanation for the rejected inline property.
    message: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "direct inline properties split presentation policy between markup and the component stylesheet",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "move the property into the paired CSS and bind only dynamic inputs with `style=(\"--name\", value)`",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            LEPTOS_STYLING_INLINE_STYLE_PROPERTIES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this presentation property is authored inline");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosStylingInlineStyleProperties: CSS custom-property boundary
// -----------------------------------------------------------------------------

/// Rejects direct inline CSS while retaining dynamic custom-property bindings.
struct LeptosStylingInlineStyleProperties {
    /// Authored source files indexed for early lint callbacks.
    files: AuthoredFiles,
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_STYLING_INLINE_STYLE_PROPERTIES,
    Warn,
    "rejects direct inline CSS properties in Leptos views",
    LeptosStylingInlineStyleProperties { files: AuthoredFiles::default() }
}

impl EarlyLintPass for LeptosStylingInlineStyleProperties {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _krate: &Crate) {
        for document in self.files.documents(cx) {
            let Some(analysis) = ComponentStyleAnalysis::analyze(document) else {
                continue;
            };
            for finding in analysis.inline_style_findings {
                Violation {
                    span: analysis.document.span(finding.range),
                    message: finding.message,
                }
                .emit(cx);
            }
        }
    }
}
