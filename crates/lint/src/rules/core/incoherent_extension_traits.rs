extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, TraitItem};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::config::LibraryConfig;
use crate::utils::extension_trait_analysis::{ExtensionTraitAnalyzer, ExtensionTraitProblem};

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
    /// ### What it does
    ///
    /// Checks local traits implemented for a foreign nominal type or a generic blanket target.
    /// Such an extension trait must operate on at most one concrete nonreceiver subject family and
    /// contain no more than `extension_traits.max_methods` methods, which defaults to eight.
    /// Receiver-only accessors count toward the method budget but do not invent a subject family.
    /// Primitive, string, and generic callback parameters likewise do not split a family.
    ///
    /// ### Why is this bad?
    ///
    /// An extension trait is useful when it gives one foreign type a focused vocabulary. A trait
    /// that accumulates unrelated subjects becomes a disguised utility module, while a large trait
    /// makes every import expose an incoherent surface and encourages agents to append the next
    /// convenient helper to the same catch-all abstraction.
    ///
    /// ```rust
    /// trait ContextExt {
    ///     fn inspect_item(&self, item: &Item<'_>);
    ///     fn inspect_expression(&self, expression: &Expr<'_>);
    /// }
    /// ```
    ///
    /// Prefer traits named and scoped around one subject:
    ///
    /// ```rust
    /// trait ItemContextExt {
    ///     fn inspect_item(&self, item: &Item<'_>);
    /// }
    ///
    /// trait ExpressionContextExt {
    ///     fn inspect_expression(&self, expression: &Expr<'_>);
    /// }
    /// ```
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
            cx.emit_span_lint(
                INCOHERENT_EXTENSION_TRAITS,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(format!(
                        "extension trait `{}` does not describe one focused capability",
                        finding.name
                    ));
                    for problem in finding.problems {
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
                    diag.help(
                        "split the API into extension traits named for one semantic subject or capability",
                    );
                }),
            );
        }
    }
}
