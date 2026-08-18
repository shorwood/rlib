extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Crate, Item};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::rules::leptos::utils::authored_files::{AuthoredFiles, SourceDocument};
use crate::rules::leptos_styling::utils::source::{ComponentStyleAnalysis, StylesheetFacts};
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Selector escaping component class scope
// -----------------------------------------------------------------------------

/// One selector branch or import that violates component CSS isolation.
struct Violation {
    /// Authored selector range receiving the diagnostic.
    span: Span,
    /// Policy-specific explanation for the escaping selector.
    message: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "unanchored or imported rules let a component stylesheet mutate unrelated document state",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "anchor every selector branch with a class declared in this file; keep classless global rules in the global base sheet",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            LEPTOS_STYLING_UNSCOPED_COMPONENT_SELECTORS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this rule escapes component class scope");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosStylingUnscopedComponentSelectors: Selector isolation policy
// -----------------------------------------------------------------------------

/// Requires every ordinary component selector branch to have a local class anchor.
struct LeptosStylingUnscopedComponentSelectors {
    /// Authored source files indexed for early lint callbacks.
    files: AuthoredFiles,
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_STYLING_UNSCOPED_COMPONENT_SELECTORS,
    Warn,
    "requires class-anchored selectors in component stylesheets",
    LeptosStylingUnscopedComponentSelectors { files: AuthoredFiles::default() }
}

impl EarlyLintPass for LeptosStylingUnscopedComponentSelectors {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _krate: &Crate) {
        for document in self.files.documents(cx) {
            let Some(analysis) = ComponentStyleAnalysis::analyze(document) else {
                continue;
            };
            if !analysis.has_view {
                continue;
            }
            let Some(style_sheet) = analysis.paired_style_sheet() else {
                continue;
            };
            let Some(path) = &style_sheet.declared else {
                continue;
            };
            let Some(css) = SourceDocument::load(cx, path) else {
                continue;
            };
            let Some(facts) = StylesheetFacts::parse(&css.source) else {
                continue;
            };
            for finding in facts.scope_findings {
                Violation {
                    span: css.span(finding.range),
                    message: finding.message,
                }
                .emit(cx);
            }
        }
    }
}
