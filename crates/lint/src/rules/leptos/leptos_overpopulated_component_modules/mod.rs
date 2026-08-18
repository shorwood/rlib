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
// Violation: Component module population diagnostic
// -----------------------------------------------------------------------------

/// Authored module containing more component declarations than policy allows.
struct Violation {
    /// Representative component declaration highlighted by the diagnostic.
    span: Span,
    /// Authored module name.
    name: String,
    /// Number of authored components in the module.
    actual: usize,
    /// Configured maximum components per module.
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

// -----------------------------------------------------------------------------
// LeptosOverpopulatedComponentModules: Module cohesion policy
// -----------------------------------------------------------------------------

/// Rejects authored modules that own too many component declarations.
struct LeptosOverpopulatedComponentModules {
    /// Project thresholds governing component architecture.
    config: LeptosArchitectureConfig,
    /// Authored Rust files accumulated across early lint callbacks.
    files: AuthoredFiles,
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_OVERPOPULATED_COMPONENT_MODULES,Warn,
    "rejects modules containing too many authored Leptos components",
    LeptosOverpopulatedComponentModules{config:ConfigStore::get().leptos_architecture.clone(),files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosOverpopulatedComponentModules {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        for module in ArchitectureAnalysis::analyze(self.files.documents(cx)).modules {
            if module.components <= self.config.max_components_per_module {
                continue;
            }
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
