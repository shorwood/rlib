extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, TraitItem};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::extension_trait_analysis::ExtensionTraitAnalyzer;

// -----------------------------------------------------------------------------
// NonAdjacentExtensionTraitImpls: Lint pass
// -----------------------------------------------------------------------------

/// Collects extension declarations and their authored implementation groups.
#[derive(Default)]
struct NonAdjacentExtensionTraitImpls {
    /// Shared semantic extension-trait analysis.
    analyzer: ExtensionTraitAnalyzer,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Checks that a local extension trait and every authored foreign-target or blanket impl form
    /// one consecutive declaration group in the same module. Comments and attributes may appear
    /// between them because they do not constitute independent module declarations.
    ///
    /// ### Why is this bad?
    ///
    /// Separating the contract from its implementation makes a small extension abstraction look
    /// like two unrelated APIs and forces readers to search for the behavior. Cross-module impls
    /// also make it easy to add an impl without discovering the trait's existing scope and naming
    /// rationale.
    ///
    /// ```rust
    /// trait ItemExt {
    ///     fn inspect(&self);
    /// }
    ///
    /// struct AnalysisState;
    ///
    /// impl ItemExt for Item<'_> {
    ///     fn inspect(&self) {}
    /// }
    /// ```
    ///
    /// Keep the complete extension beside its declaration:
    ///
    /// ```rust
    /// trait ItemExt {
    ///     fn inspect(&self);
    /// }
    ///
    /// impl ItemExt for Item<'_> {
    ///     fn inspect(&self) {}
    /// }
    ///
    /// struct AnalysisState;
    /// ```
    pub NON_ADJACENT_EXTENSION_TRAIT_IMPLS,
    Warn,
    "enforces one adjacent declaration group for extension traits and their authored impls",
    NonAdjacentExtensionTraitImpls::default()
}

impl LateLintPass<'_> for NonAdjacentExtensionTraitImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.analyzer.record_item(cx, item);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'_>, item: &TraitItem<'_>) {
        self.analyzer.record_trait_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for finding in self.analyzer.placement_findings() {
            cx.emit_span_lint(
                NON_ADJACENT_EXTENSION_TRAIT_IMPLS,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(format!(
                        "the impl group for extension trait `{}` is not adjacent to its declaration",
                        finding.name
                    ));
                    diag.span_label(finding.trait_span, "extension trait declared here");
                    diag.help(
                        "move every authored extension impl into one consecutive group immediately after the trait in the same module",
                    );
                }),
            );
        }
    }
}
