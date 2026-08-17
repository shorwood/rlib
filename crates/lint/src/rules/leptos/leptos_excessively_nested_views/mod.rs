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
    depth: usize,
    maximum: usize,
    control: usize,
    control_maximum: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "component `{}` has view depth {} and embedded control depth {}; limits are {} and {}",
            self.name, self.depth, self.control, self.maximum, self.control_maximum
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "deep markup and control layers conceal the component's visual hierarchy and states",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "flatten redundant wrappers and extract a cohesive named child component at a semantic boundary",
        )
    }
    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_EXCESSIVELY_NESTED_VIEWS,
            self.span,
            DiagDecorator(|d| {
                d.primary_message(p);
                d.note(r);
                d.help(h);
            }),
        );
    }
}

struct LeptosExcessivelyNestedViews {
    config: LeptosArchitectureConfig,
    files: AuthoredFiles,
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_EXCESSIVELY_NESTED_VIEWS,Warn,
    "rejects excessively nested authored Leptos views",
    LeptosExcessivelyNestedViews{config:LeptosArchitectureConfig::from_config(),files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosExcessivelyNestedViews {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }
    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        for f in ArchitectureAnalysis::analyze(self.files.documents(cx)).functions {
            if f.is_component
                && (f.max_view_depth > self.config.max_view_nesting_depth
                    || f.max_view_control_depth > self.config.max_view_control_depth)
            {
                Violation {
                    span: f.span,
                    name: f.name,
                    depth: f.max_view_depth,
                    maximum: self.config.max_view_nesting_depth,
                    control: f.max_view_control_depth,
                    control_maximum: self.config.max_view_control_depth,
                }
                .emit(cx);
            }
        }
    }
}
