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
// Violation: Component stylesheet ownership mismatch
// -----------------------------------------------------------------------------

/// One component source that does not own exactly its paired stylesheet.
struct Violation {
    /// Component or stylesheet source range receiving the ownership diagnostic.
    span: Span,
    /// Concrete ownership mismatch found for this component source.
    message: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "colocated one-to-one ownership makes a component's styling dependencies explicit",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "declare exactly `style_sheet!(style, \"<module>.css\", ...)` in the styled component module",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            LEPTOS_STYLING_NON_COLOCATED_COMPONENT_STYLES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "stylesheet ownership is not local and one-to-one",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosStylingNonColocatedComponentStyles: Component stylesheet pairing policy
// -----------------------------------------------------------------------------

/// Enforces strict external stylesheet ownership for styled component files.
struct LeptosStylingNonColocatedComponentStyles {
    /// Authored source files containing styling macros, deduplicated across callbacks.
    files: AuthoredFiles,
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_STYLING_NON_COLOCATED_COMPONENT_STYLES,
    Warn,
    "requires styled Leptos modules to own one same-named CSS file",
    LeptosStylingNonColocatedComponentStyles { files: AuthoredFiles::default() }
}

impl EarlyLintPass for LeptosStylingNonColocatedComponentStyles {
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

            for range in &analysis.inline_style_sheets {
                Violation {
                    span: analysis.document.span(range.clone()),
                    message: "component styles must not use inline stylesheet macros".to_owned(),
                }
                .emit(cx);
            }

            if !analysis.is_styled && analysis.style_sheets.is_empty() {
                continue;
            }
            if analysis.style_sheets.len() != 1 {
                Violation {
                    span: analysis.document.complete_span(),
                    message: format!(
                        "styled component declares {} external stylesheets instead of exactly one",
                        analysis.style_sheets.len()
                    ),
                }
                .emit(cx);
                continue;
            }

            let style_sheet = &analysis.style_sheets[0];
            if style_sheet.alias.as_deref() != Some("style") {
                Violation {
                    span: analysis.document.span(style_sheet.range.clone()),
                    message: "component stylesheet must expose its generated constants as `style`"
                        .to_owned(),
                }
                .emit(cx);
            }
            if analysis.paired_style_sheet().is_none() {
                Violation {
                    span: analysis.document.span(style_sheet.range.clone()),
                    message: format!(
                        "component stylesheet must be the paired file `{}`",
                        analysis.paired_css_path().display()
                    ),
                }
                .emit(cx);
                continue;
            }

            let paired = analysis.paired_css_path();
            let Some(css) = SourceDocument::load(cx, &paired) else {
                Violation {
                    span: analysis.document.span(style_sheet.range.clone()),
                    message: format!("paired stylesheet `{}` does not exist", paired.display()),
                }
                .emit(cx);
                continue;
            };

            // Only a successfully parsed stylesheet with zero classes violates this final check.
            if StylesheetFacts::parse(&css.source).is_none_or(|facts| !facts.classes.is_empty()) {
                continue;
            }
            Violation {
                span: css.complete_span(),
                message: "paired component stylesheet contains no class selectors".to_owned(),
            }
            .emit(cx);
        }
    }
}
