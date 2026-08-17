extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Crate, Item};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::rules::leptos::utils::authored_files::AuthoredFiles;
use crate::rules::leptos::utils::component_architecture::ArchitectureAnalysis;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Reactive helper naming diagnostic
// -----------------------------------------------------------------------------

/// Reactive helper whose lifecycle ownership is hidden by an ordinary function name.
struct Violation {
    /// Authored helper declaration highlighted by the diagnostic.
    span: Span,
    /// Authored helper name missing the composable prefix.
    name: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "reactive helper `{}` is not named as a composable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "reactive ownership and lifecycle behavior should be visible at the call site",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "rename `{}` with a `use_` prefix that names the owned responsibility",
            self.name
        ))
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_UNNAMED_COMPOSABLES,
            self.span,
            DiagDecorator(|d| {
                d.primary_message(p);
                d.note(r);
                d.help(h);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosUnnamedComposables: Reactive helper naming policy
// -----------------------------------------------------------------------------

/// Collects authored helpers and rejects reactive owners without composable names.
struct LeptosUnnamedComposables {
    /// Authored Rust files accumulated across early lint callbacks.
    files: AuthoredFiles,
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNNAMED_COMPOSABLES,Warn,
    "requires project-local reactive helpers to use composable naming",
    LeptosUnnamedComposables{files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosUnnamedComposables {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        let analysis = ArchitectureAnalysis::analyze(self.files.documents(cx));
        for finding in analysis.unnamed_composables() {
            Violation {
                span: finding.span,
                name: finding.name,
            }
            .emit(cx);
        }
    }
}
