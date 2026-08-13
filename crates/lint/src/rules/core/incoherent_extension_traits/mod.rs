extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, TraitItem};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Span, Symbol};
use serde::Deserialize;

use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;
use crate::utils::extension_trait_analysis::{ExtensionTraitAnalyzer, ExtensionTraitProblem};

// -----------------------------------------------------------------------------
// ExtensionTraitConfig: Focused extension trait limits
// -----------------------------------------------------------------------------

/// Limits that prevent extension traits from becoming catch-all APIs.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct ExtensionTraitConfig {
    /// Maximum methods permitted in one focused extension trait.
    pub(crate) max_methods: usize,
}

impl Default for ExtensionTraitConfig {
    fn default() -> Self {
        Self { max_methods: 8 }
    }
}

impl ExtensionTraitConfig {
    fn validate(&self) -> Result<(), String> {
        if self.max_methods == 0 {
            return Err("extension_traits.max_methods must be greater than zero".to_owned());
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Violation: Incoherent extension trait diagnostic
// -----------------------------------------------------------------------------

/// Extension trait whose subjects or method count exceed one focused capability.
struct Violation {
    /// Trait name span used as the primary diagnostic location.
    span: Span,
    /// Trait name shown in the diagnostic.
    name: Symbol,
    /// Concrete coherence failures found during crate-wide analysis.
    problems: Vec<ExtensionTraitProblem>,
    /// Configured method budget needed to explain oversized traits precisely.
    max_methods: usize,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "extension trait `{}` does not describe one focused capability",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "importing `{}` exposes unrelated or oversized behavior as though it were one coherent extension API",
            self.name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "split the API into extension traits named for one semantic subject or capability",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render the stable diagnostic layers before moving problem-specific evidence.
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();

        // Emit the stable layers before consuming the problem-specific evidence.
        cx.emit_span_lint(
            INCOHERENT_EXTENSION_TRAITS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                for problem in self.problems {
                    match problem {
                        ExtensionTraitProblem::MultipleSubjects(subjects) => {
                            let subjects = subjects
                                .iter()
                                .map(|subject| format!("`{subject}`"))
                                .collect::<Vec<_>>()
                                .join(", ");
                            diag.note(format!(
                                "its methods operate on multiple concrete subject families: {subjects}"
                            ));
                        }
                        ExtensionTraitProblem::TooManyMethods(count) => {
                            diag.note(format!(
                                "it defines {count} methods, exceeding the configured limit of {}",
                                self.max_methods
                            ));
                        }
                    }
                }
                diag.note(rationale_message);
                diag.help(remediation_message);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// IncoherentExtensionTraits: Lint pass
// -----------------------------------------------------------------------------

/// Collects extension traits and checks their subject and size boundaries.
struct IncoherentExtensionTraits {
    /// Shared semantic extension-trait analysis.
    analyzer: ExtensionTraitAnalyzer,
    /// Maximum methods permitted before a trait must be decomposed.
    max_methods: usize,
}

impl IncoherentExtensionTraits {
    /// Builds the pass from validated extension-trait configuration.
    fn new() -> Self {
        // Reject invalid limits before constructing the shared semantic analyzer.
        let config = LibraryConfig::load().extension_traits;
        config
            .validate()
            .unwrap_or_else(|message| panic!("invalid extension trait configuration: {message}"));

        // Couple the validated method budget with a fresh crate analysis.
        Self {
            analyzer: ExtensionTraitAnalyzer::default(),
            max_methods: config.max_methods,
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub INCOHERENT_EXTENSION_TRAITS,
    Warn,
    "detects extension traits that mix subject families or grow beyond a focused method budget",
    IncoherentExtensionTraits::new()
}

impl LateLintPass<'_> for IncoherentExtensionTraits {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.analyzer.record_item(cx, item);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'_>, item: &TraitItem<'_>) {
        self.analyzer.record_trait_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for finding in self.analyzer.coherence_findings(cx, self.max_methods) {
            Violation {
                span: finding.span,
                name: finding.name,
                problems: finding.problems,
                max_methods: self.max_methods,
            }
            .emit(cx);
        }
    }
}

#[cfg(test)]
mod config_tests {
    use super::ExtensionTraitConfig;

    #[test]
    fn uses_eight_methods_by_default() {
        assert_eq!(ExtensionTraitConfig::default().max_methods, 8);
    }

    #[test]
    fn rejects_zero_methods() {
        assert!(ExtensionTraitConfig { max_methods: 0 }.validate().is_err());
    }
}
