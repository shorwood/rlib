extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::BonAttributeAnalysis;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Builder retaining a positional-heavy contract
// -----------------------------------------------------------------------------

/// Public builder whose entry and finish functions carry too many inputs.
struct Violation {
    /// Builder attribute receiving the diagnostic.
    span: Span,
    /// Members kept positional by start or finish policy.
    positional: usize,
    /// Total members exposed by the builder.
    total: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Bon builder keeps {} of {} members positional",
            self.positional, self.total
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a parameter-heavy builder loses its named-argument value when most inputs remain positional at its start or finish boundary",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "keep only a small identity-bearing prefix or terminal value positional and expose the remaining inputs as named setters",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_REDUNDANT_POSITIONAL_AND_BUILDER_APIS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this builder retains a positional-heavy call contract",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// BonRedundantPositionalAndBuilderApis: Named-argument policy
// -----------------------------------------------------------------------------

/// Ensures large public builders retain the readability benefit of named setters.
struct BonRedundantPositionalAndBuilderApis;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_REDUNDANT_POSITIONAL_AND_BUILDER_APIS,
    Warn,
    "rejects positional-heavy public Bon builders",
    BonRedundantPositionalAndBuilderApis
}

impl BonRedundantPositionalAndBuilderApis {
    /// Minimum total members that make positional builder entry points hard to read.
    const MINIMUM_TOTAL_MEMBERS: usize = 5;

    /// Minimum positional members required before evaluating their share.
    const MINIMUM_POSITIONAL_MEMBERS: usize = 3;

    /// Denominator for the maximum one-half positional share.
    const POSITIONAL_SHARE_DENOMINATOR: usize = 2;
}
impl EarlyLintPass for BonRedundantPositionalAndBuilderApis {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        let ItemKind::Fn(function) = &item.kind else {
            return;
        };
        let Some(attribute) = BonAttributeAnalysis::builder(&item.attrs) else {
            return;
        };

        if !matches!(item.vis.kind, VisibilityKind::Public) {
            return;
        }
        let total = function.sig.decl.inputs.len();

        let positional = function
            .sig
            .decl
            .inputs
            .iter()
            .filter(|parameter| {
                BonAttributeAnalysis::builder_contains(cx, &parameter.attrs, "start_fn")
                    || BonAttributeAnalysis::builder_contains(cx, &parameter.attrs, "finish_fn")
            })
            .count();

        // Small or predominantly named APIs retain a useful builder contract.
        if !(total >= Self::MINIMUM_TOTAL_MEMBERS
            && positional >= Self::MINIMUM_POSITIONAL_MEMBERS
            && positional * Self::POSITIONAL_SHARE_DENOMINATOR >= total)
        {
            return;
        }
        Violation {
            span: attribute.span,
            positional,
            total,
        }
        .emit(cx);
    }
}
