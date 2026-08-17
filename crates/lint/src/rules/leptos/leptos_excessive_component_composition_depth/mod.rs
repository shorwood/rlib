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
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "component `{}` begins a composition chain of depth {}; the limit is {}",
            self.name, self.depth, self.maximum
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "long chains of single-purpose wrappers scatter one screen's structure across too many navigation hops",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "collapse pass-through layers or combine children that do not own an independent UI responsibility",
        )
    }
    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_EXCESSIVE_COMPONENT_COMPOSITION_DEPTH,
            self.span,
            DiagDecorator(|d| {
                d.primary_message(p);
                d.note(r);
                d.help(h);
            }),
        );
    }
}

struct LeptosExcessiveComponentCompositionDepth {
    config: LeptosArchitectureConfig,
    files: AuthoredFiles,
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_EXCESSIVE_COMPONENT_COMPOSITION_DEPTH,Warn,
    "rejects overlong local Leptos component composition chains",
    LeptosExcessiveComponentCompositionDepth{config:LeptosArchitectureConfig::from_config(),files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosExcessiveComponentCompositionDepth {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }
    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        let analysis = ArchitectureAnalysis::analyze(self.files.documents(cx));
        for finding in analysis.excessive_compositions(self.config.max_component_composition_depth)
        {
            Violation {
                span: finding.span,
                name: finding.name,
                depth: finding.depth,
                maximum: self.config.max_component_composition_depth,
            }
            .emit(cx);
        }
    }
}
