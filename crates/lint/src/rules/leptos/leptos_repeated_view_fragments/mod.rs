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
    ArchitectureAnalysis, LeptosArchitectureConfig, RepeatedFragmentPolicy,
};
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Repeated view fragment diagnostic
// -----------------------------------------------------------------------------

/// Repeated normalized RSX fragment large enough to require extraction.
struct Violation {
    /// Repeated occurrence highlighted by the primary diagnostic.
    span: Span,
    /// First normalized occurrence used as supporting evidence.
    first: Span,
    /// Number of meaningful nodes in the repeated fragment.
    nodes: usize,
    /// Number of non-overlapping normalized occurrences.
    occurrences: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "repeated view fragment has {} meaningful nodes across {} occurrences",
            self.nodes, self.occurrences
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "copied RSX drifts independently and makes the shared interface or visual concept implicit",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "extract the maximal repeated fragment into a named component with explicit props",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        let first = self.first;
        cx.emit_span_lint(
            LEPTOS_REPEATED_VIEW_FRAGMENTS,
            self.span,
            DiagDecorator(|d| {
                d.primary_message(p);
                d.span_label(first, "first normalized occurrence is here");
                d.note(r);
                d.help(h);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosRepeatedViewFragments: Authored view reuse policy
// -----------------------------------------------------------------------------

/// Rejects repeated normalized RSX that should become a named component.
struct LeptosRepeatedViewFragments {
    /// Project thresholds governing component architecture.
    config: LeptosArchitectureConfig,
    /// Authored Rust files accumulated across early lint callbacks.
    files: AuthoredFiles,
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_REPEATED_VIEW_FRAGMENTS,Warn,
    "rejects repeated normalized Leptos view fragments",
    LeptosRepeatedViewFragments{config:LeptosArchitectureConfig::from_config(),files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosRepeatedViewFragments {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        let analysis = ArchitectureAnalysis::analyze(self.files.documents(cx));
        let policy = RepeatedFragmentPolicy {
            minimum_nodes: self.config.min_repeated_fragment_nodes,
            minimum_occurrences: self.config.min_repeated_fragment_occurrences,
        };
        for finding in analysis.repeated_fragments(policy) {
            Violation {
                span: finding.span,
                first: finding.first_span,
                nodes: finding.nodes,
                occurrences: finding.occurrences,
            }
            .emit(cx);
        }
    }
}
