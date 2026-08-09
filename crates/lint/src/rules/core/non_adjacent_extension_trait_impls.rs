extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, TraitItem};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::extension_trait_analysis::ExtensionTraitAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Separated extension trait group diagnostic
// -----------------------------------------------------------------------------

/// Extension trait whose declaration and authored impls are separated.
struct Violation {
    /// First misplaced impl, or trait span for a cross-module group.
    span: Span,
    /// Trait declaration span shown as the related location.
    trait_span: Span,
    /// Extension trait name shown in every diagnostic layer.
    name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the impl group for extension trait `{}` is not adjacent to its declaration",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the contract and implementation of `{}` cannot be reviewed as one consecutive capability",
            self.name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "move every authored extension impl into one consecutive group immediately after the trait in the same module",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            NON_ADJACENT_EXTENSION_TRAIT_IMPLS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.trait_span, "extension trait declared here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

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
            Violation {
                span: finding.span,
                trait_span: finding.trait_span,
                name: finding.name,
            }
            .emit(cx);
        }
    }
}
