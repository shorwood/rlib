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
            "module `{}` defines {} components; the limit is {}",
            self.name, self.actual, self.maximum
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "crowded component modules blur ownership and make unrelated UI responsibilities change together",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("split the module along cohesive feature or visual responsibilities")
    }
    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_OVERPOPULATED_COMPONENT_MODULES,
            self.span,
            DiagDecorator(|d| {
                d.primary_message(p);
                d.note(r);
                d.help(h);
            }),
        );
    }
}

struct LeptosOverpopulatedComponentModules {
    config: LeptosArchitectureConfig,
    files: AuthoredFiles,
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_OVERPOPULATED_COMPONENT_MODULES,Warn,
    "rejects modules containing too many authored Leptos components",
    LeptosOverpopulatedComponentModules{config:LeptosArchitectureConfig::from_config(),files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosOverpopulatedComponentModules {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }
    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        for module in ArchitectureAnalysis::analyze(self.files.documents(cx)).modules {
            if module.components > self.config.max_components_per_module {
                Violation {
                    span: module.span,
                    name: module.name,
                    actual: module.components,
                    maximum: self.config.max_components_per_module,
                }
                .emit(cx);
            }
        }
    }
}
