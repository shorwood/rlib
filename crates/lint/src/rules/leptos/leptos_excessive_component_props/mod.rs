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
// Violation: Component property count diagnostic
// -----------------------------------------------------------------------------

/// Component whose authored non-children property surface exceeds policy.
struct Violation {
    /// Component declaration highlighted by the diagnostic.
    span: Span,
    /// Authored component name.
    name: String,
    /// Number of authored non-children properties.
    actual: usize,
    /// Configured maximum component properties.
    maximum: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "component `{}` has {} non-children props; the limit is {}",
            self.name, self.actual, self.maximum
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "large prop surfaces expose internal coordination and make call sites difficult to understand",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "group cohesive domain input into a typed value or split the component by independent responsibility",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_EXCESSIVE_COMPONENT_PROPS,
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
// LeptosExcessiveComponentProps: Component api policy
// -----------------------------------------------------------------------------

/// Rejects components with excessive authored property surfaces.
struct LeptosExcessiveComponentProps {
    /// Project thresholds governing component architecture.
    config: LeptosArchitectureConfig,
    /// Authored Rust files accumulated across early lint callbacks.
    files: AuthoredFiles,
}

crate::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_EXCESSIVE_COMPONENT_PROPS,Warn,
    "rejects Leptos components with excessive prop surfaces",
    LeptosExcessiveComponentProps{config:ConfigStore::get().leptos_architecture.clone(),files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosExcessiveComponentProps {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        for function in ArchitectureAnalysis::analyze(self.files.documents(cx)).functions {
            if !function.is_component || function.props <= self.config.max_component_props {
                continue;
            }
            Violation {
                span: function.span,
                name: function.name,
                actual: function.props,
                maximum: self.config.max_component_props,
            }
            .emit(cx);
        }
    }
}
