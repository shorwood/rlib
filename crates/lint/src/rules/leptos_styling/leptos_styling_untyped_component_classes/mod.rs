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
// Violation: Untyped authored class value
// -----------------------------------------------------------------------------

/// One class-bearing view attribute that bypasses local generated constants.
struct Violation {
    /// Authored class attribute range receiving the diagnostic.
    span: Span,
    /// Policy-specific explanation for the untyped class source.
    message: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "raw or opaque classes are not rewritten by Turf and cannot prove local stylesheet ownership",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "compose the class from this module's `style::CONSTANT` values; an empty conditional fallback is allowed",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            LEPTOS_STYLING_UNTYPED_COMPONENT_CLASSES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this class source is not locally typed");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosStylingUntypedComponentClasses: Generated constant policy
// -----------------------------------------------------------------------------

/// Requires authored view classes to use the colocated sheet's generated API.
struct LeptosStylingUntypedComponentClasses {
    /// Authored source files indexed for early lint callbacks.
    files: AuthoredFiles,
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_STYLING_UNTYPED_COMPONENT_CLASSES,
    Warn,
    "rejects Leptos classes that bypass local generated stylesheet constants",
    LeptosStylingUntypedComponentClasses { files: AuthoredFiles::default() }
}

impl EarlyLintPass for LeptosStylingUntypedComponentClasses {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _krate: &Crate) {
        for document in self.files.documents(cx) {
            let Some(analysis) = ComponentStyleAnalysis::analyze(document) else {
                continue;
            };
            for finding in analysis.class_findings {
                Violation {
                    span: analysis.document.span(finding.range),
                    message: finding.message,
                }
                .emit(cx);
            }
        }
    }
}
