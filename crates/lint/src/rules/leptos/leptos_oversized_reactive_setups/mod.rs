extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Crate, Item};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::config::leptos::LeptosArchitectureConfig;
use crate::config::store::ConfigStore;
use crate::rules::leptos::utils::authored_files::AuthoredFiles;
use crate::rules::leptos::utils::component_architecture::ArchitectureAnalysis;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Reactive setup size diagnostic
// -----------------------------------------------------------------------------

/// Reactive owner whose setup phase exceeds the configured statement limit.
struct Violation {
    /// Component or composable declaration highlighted by the diagnostic.
    span: Span,
    /// Authored reactive-owner name.
    name: String,
    /// Number of authored setup statements.
    actual: usize,
    /// Configured maximum setup statements.
    maximum: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "reactive setup `{}` has {} statements; the limit is {}",
            self.name, self.actual, self.maximum
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "large setup phases mix unrelated state, effects, loading, and mutation responsibilities",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("extract cohesive reactive responsibilities into named `use_*` composables")
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_OVERSIZED_REACTIVE_SETUPS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.note(rationale);
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosOversizedReactiveSetups: Reactive setup policy
// -----------------------------------------------------------------------------

/// Rejects components and composables with oversized setup phases.
struct LeptosOversizedReactiveSetups {
    /// Project thresholds governing component architecture.
    config: LeptosArchitectureConfig,
    /// Authored Rust files accumulated across early lint callbacks.
    files: AuthoredFiles,
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_OVERSIZED_REACTIVE_SETUPS, Warn,
    "rejects oversized setup phases in Leptos components and composables",
    LeptosOversizedReactiveSetups { config: ConfigStore::get().leptos_architecture.clone(), files: AuthoredFiles::default() }
}

impl EarlyLintPass for LeptosOversizedReactiveSetups {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _krate: &Crate) {
        for function in ArchitectureAnalysis::analyze(self.files.documents(cx)).functions {
            if !function.is_reactive_owner()
                || function.setup_statements <= self.config.max_setup_statements
            {
                continue;
            }
            Violation {
                span: function.span,
                name: function.name,
                actual: function.setup_statements,
                maximum: self.config.max_setup_statements,
            }
            .emit(cx);
        }
    }
}
