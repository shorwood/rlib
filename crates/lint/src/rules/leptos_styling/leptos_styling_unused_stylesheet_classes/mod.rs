extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Crate, Item};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::rules::leptos::utils::authored_files::{AuthoredFiles, SourceDocument};
use crate::rules::leptos_styling::utils::source::{
    ComponentStyleAnalysis, StylesheetFacts, generated_constant,
};
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Unreferenced generated class
// -----------------------------------------------------------------------------

/// One component class with no semantic `style::CONSTANT` reference.
struct Violation {
    /// Authored class selector range receiving the diagnostic.
    span: Span,
    /// Unreferenced stylesheet class name without its selector prefix.
    class: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "stylesheet class `.{}` is not used by its component module",
            self.class
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "unused component selectors accumulate dead presentation policy and conceal stale markup",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "remove `.{}` or reference it through `style::{}`",
            self.class,
            generated_constant(&self.class)
        ))
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            LEPTOS_STYLING_UNUSED_STYLESHEET_CLASSES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this generated class is never referenced");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosStylingUnusedStylesheetClasses: Local dead-style policy
// -----------------------------------------------------------------------------

/// Finds CSS classes without typed references in their owning Rust module.
struct LeptosStylingUnusedStylesheetClasses {
    /// Authored source files indexed for early lint callbacks.
    files: AuthoredFiles,
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_STYLING_UNUSED_STYLESHEET_CLASSES,
    Warn,
    "rejects unused classes in colocated Leptos stylesheets",
    LeptosStylingUnusedStylesheetClasses { files: AuthoredFiles::default() }
}

impl EarlyLintPass for LeptosStylingUnusedStylesheetClasses {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _krate: &Crate) {
        for document in self.files.documents(cx) {
            let Some(analysis) = ComponentStyleAnalysis::analyze(document) else {
                continue;
            };
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
            for class in facts.classes {
                if analysis
                    .class_references
                    .contains(&generated_constant(&class.name))
                {
                    continue;
                }
                Violation {
                    span: css.span(class.range),
                    class: class.name,
                }
                .emit(cx);
            }
        }
    }
}
