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
// Violation: Detached task ownership diagnostic
// -----------------------------------------------------------------------------

/// Reactive owner that starts a task outside a lifecycle-aware primitive.
struct Violation {
    /// Spawn expression highlighted by the diagnostic.
    span: Span,
    /// Component or composable owning the detached task.
    name: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!("`{}` starts an unscoped local task", self.name))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a detached task hides cancellation, ownership, mutation, and loading semantics from the reactive graph",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use an Action for mutation, a Resource for loading, or an owner-cleaned `use_*` composable for lifetime work",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_UNSCOPED_SPAWNED_TASKS,
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
// LeptosUnscopedSpawnedTasks: Reactive task ownership policy
// -----------------------------------------------------------------------------

/// Collects authored reactive owners and rejects detached local tasks.
struct LeptosUnscopedSpawnedTasks {
    /// Authored Rust files accumulated across early lint callbacks.
    files: AuthoredFiles,
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNSCOPED_SPAWNED_TASKS,Warn,
    "rejects detached local tasks in Leptos components and composables",
    LeptosUnscopedSpawnedTasks{files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosUnscopedSpawnedTasks {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        for f in ArchitectureAnalysis::analyze(self.files.documents(cx)).functions {
            if !f.is_reactive_owner() {
                continue;
            }
            for span in f.spawned_tasks {
                Violation {
                    span,
                    name: f.name.clone(),
                }
                .emit(cx);
            }
        }
    }
}
