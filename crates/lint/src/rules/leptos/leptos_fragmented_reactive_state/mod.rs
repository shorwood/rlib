extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Crate, Item};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::rules::leptos::utils::authored_files::AuthoredFiles;
use crate::rules::leptos::utils::component_architecture::{
    ArchitectureAnalysis, LeptosArchitectureConfig,
};
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Reactive primitive count diagnostic
// -----------------------------------------------------------------------------

/// Reactive owner that directly creates too many independent primitives.
struct Violation {
    /// Component or composable declaration highlighted by the diagnostic.
    span: Span,
    /// Authored reactive-owner name.
    name: String,
    /// Number of directly created reactive primitives.
    actual: usize,
    /// Configured maximum reactive primitives.
    maximum: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` directly creates {} reactive primitives; the limit is {}",
            self.name, self.actual, self.maximum
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "fragmented primitives obscure the cohesive state transitions owned by a component or composable",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "model related state as one domain draft or split independent responsibilities across focused `use_*` composables",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_FRAGMENTED_REACTIVE_STATE,
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
// LeptosFragmentedReactiveState: Reactive state policy
// -----------------------------------------------------------------------------

/// Rejects reactive owners whose directly created state is excessively fragmented.
struct LeptosFragmentedReactiveState {
    /// Project thresholds governing component architecture.
    config: LeptosArchitectureConfig,
    /// Authored Rust files accumulated across early lint callbacks.
    files: AuthoredFiles,
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_FRAGMENTED_REACTIVE_STATE, Warn,
    "rejects components and composables with fragmented reactive state",
    LeptosFragmentedReactiveState { config: LeptosArchitectureConfig::from_config(), files: AuthoredFiles::default() }
}
impl EarlyLintPass for LeptosFragmentedReactiveState {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        for function in ArchitectureAnalysis::analyze(self.files.documents(cx)).functions {
            if !function.is_reactive_owner()
                || function.reactive_primitives.len() <= self.config.max_reactive_primitives
            {
                continue;
            }
            Violation {
                span: function.span,
                name: function.name,
                actual: function.reactive_primitives.len(),
                maximum: self.config.max_reactive_primitives,
            }
            .emit(cx);
        }
    }
}
