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

struct Violation {
    span: Span,
    name: String,
    actual: usize,
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

struct LeptosFragmentedReactiveState {
    config: LeptosArchitectureConfig,
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
        for f in ArchitectureAnalysis::analyze(self.files.documents(cx)).functions {
            if f.is_reactive_owner()
                && f.reactive_primitives.len() > self.config.max_reactive_primitives
            {
                Violation {
                    span: f.span,
                    name: f.name,
                    actual: f.reactive_primitives.len(),
                    maximum: self.config.max_reactive_primitives,
                }
                .emit(cx);
            }
        }
    }
}
